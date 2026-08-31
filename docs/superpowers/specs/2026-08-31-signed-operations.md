# Signed Operations

**Status:** implemented in `groundhog-domain-separator`, `groundhog-signed-op-if`,
and `groundhog-test-vectors`.

**Audience:** contract authors, and anyone implementing the client half.

**`hogd` needs no changes.** A signed operation is verified *inside a contract*,
using the `SHA256` and `VerifySignature` syscalls that already exist, and the
contract then mints with `MintAuthToken`. The node enforces the auth-token tree
it already enforces and never parses an operation preimage. The two parties that
must agree on these bytes are the Rust contract and the TypeScript client — which
is what the cross-SDK vectors pin. (Contrast `transaction_signature_preimage`,
which genuinely *is* a node-side layout.) The one item here that would touch
`hogd` is the optional `GetNetworkHash` syscall in §6, and it is not part of this
work.

## 1. What this is

A *signed operation* is a structured message, signed off-chain, that authorizes
an on-chain action. The signer need not be the submitter, which is the pattern
behind permits, session keys, meta-transactions and delegated wallet actions.

Ethereum's answer is EIP-712, and the vocabulary transfers even though none of
the bytes do. EIP-712 is three things at once, and Groundhog already had most:

| EIP-712 | Groundhog | Before this change |
|---|---|---|
| `keccak256` | `sha256` (syscall + host backends) | ✅ |
| ABI encoding | XDR / CXS | ✅ |
| `\x19\x01` envelope | CXS domain-separator prefix | ✅ |
| `EIP712Domain.name` | `DomainSeparator.domain_separator` | ✅ |
| `EIP712Domain.version` | — | ❌ added (§2) |
| `EIP712Domain.chainId` | `NetworkHash` | ✅ transactions only |
| `EIP712Domain.verifyingContract` | — | ❌ added (§3) |
| `typeHash` / `encodeType` | `ContractTypeSpec::TYPE_DEF` | ✅ existed, unused for signing (§4) |
| nonce, deadline | `Uniquifier` | ✅ transactions only |
| ERC-1271 `isValidSignature` | `groundhog-erc1271-if` | ✅ |
| `eth_signTypedData_v4` | — | ❌ client-side, §8 |

The name "EIP-712" is deliberately **not** used: Groundhog is
sha256/ed25519/XDR where EIP-712 is keccak256/secp256k1/ABI, so borrowing the
number would promise byte- and signer-level compatibility that does not exist.
ERC-1271 keeps its name because it is an interface shape, not a crypto binding.

## 2. Domain separator versioning

`domain_separator!` declares a separator whose CXS prefix is computed in a
`const` and lands in rodata:

```rust
domain_separator! {
    pub const MY_APP = "MyApp" version "1";
}
// MY_APP.as_bytes() == b"\x00\x08MyApp::1"
```

The version is folded into the protocol's existing `opaque<256>` name field
rather than becoming a new struct field. This is not cosmetic:
`groundhog_xdr::preimage::DomainSeparator` is a protocol type shared with the
C++ validator, and a new field would change the encoding of *every* separator —
moving every pubkey account address and invalidating every in-flight
transaction signature. `crates/domain-separator/src/separator.rs` keeps a mirror
of the protocol type plus a drift guard (`prefix_matches_real_domain_separator`)
that fails if the two diverge.

`::` is the same delimiter, for the same reason, as in entrypoint IDs and event
signatures: without it `("MyApp1", "2")` and `("MyApp", "12")` share a domain.

**The separator version is the application's schema epoch.** Bumping it retires
every signature made under the old schema at once. §4 explains what it has to
cover that the type binding does not.

## 3. The preimage

```text
sha256 is NOT applied before signing. These bytes are signed directly.

  cxs(DomainSeparator("SignedOp::1"))   13 bytes: 00 0b "SignedOp::1"
  network                               32 bytes, verbatim
  verifying_contract                    32 bytes, verbatim
  cxs(T::TYPE_DEF)                      the payload type's definition (§4)
  nonce                                 CXS varint (u64), 1–9 bytes
  valid_until                           CXS varint (i64), 1–9 bytes
  cxs(body)                             the payload value, fields inlined
```

Notes for implementers:

- **Only the first 64 bytes are fixed-width.** Everything after is
  variable-length but self-delimiting: CXS varints carry their own length, and
  the type definition and body are structured. Do not assume 8-byte integers —
  `nonce = 1` is one byte.
- **Varint canonicality is not a concern**, because the preimage is only ever
  *constructed*, never parsed. Both sides build it from typed values and
  re-encode. A verifier that accepted preimage bytes over the wire would have to
  reject non-canonical varints itself.
- **The body is a typed value, not opaque bytes.** `check_signed_op::<T>`
  decodes the wire bytes as `T` first and rebuilds the preimage from the decoded
  value. That canonicalises: a non-canonical encoding that happens to decode
  re-encodes to what the signer actually signed, or fails verification.
- **No pre-hash.** Ed25519 hashes its own message, so signing a digest would add
  an on-chain SHA-256 and a second convention for no cryptographic gain, and
  `transaction_signature_preimage` already set the convention. `SignedOp::hash()`
  exists as a stable *identifier* (nonce records, logs, dedup) and is what the
  vectors compare, but it is not what gets signed.

Each field closes exactly one replay path, and none is optional:

| Field | Without it, the signature replays |
|---|---|
| `network` | on another network carrying the same address |
| `verifying_contract` | against a different contract |
| `T::TYPE_DEF` | as a different kind of message |
| `nonce` / `valid_until` | again, or forever |

## 4. The type binding

There is no operation-type identifier. The binding is the payload type's own
definition, which `#[contract_type]` already emits:

```rust
impl ContractTypeSpec for Transfer {
    const TYPE_DEF: ContractTypeDefView<'static> = /* name, field names, field types */;
}
```

`ContractTypeDefView` derives `ConstWriteXDR`, so this is a const the compiler
puts in rodata — the same machinery the separator prefix uses.

### Why it is necessary

CXS concatenates struct fields with no framing — no type tag, no field count, no
length prefix. `Grant { amount: u64 }` and `Revoke { id: u64 }` encode
identically, so without a type binding a signature over one *is* a signature
over the other. It bites hardest with auth tokens, where `Signed<T>` fields
encode to **zero bytes**: two authorizations differing only in which contract
their children are addressed to have byte-identical payloads. Node-level safety
comes from `AuthTokenNode.consumer_address`, but that is enforced at consume
time, not at signature time.

### Why the type definition rather than a name or an ID

This is EIP-712's `encodeType` with its worst footgun removed. EIP-712's type
string is hand-written next to the struct and drifts from it; this one is
generated from the struct. It is also structural rather than nominal, so adding
or renaming a field of `T` changes the preimage and retires old signatures. A
name, or a hash of `Interface::Major::name`, would not — it would keep accepting
signatures made against the old shape.

It also means **there is no check to skip.** A verifier reconstructs the
preimage from the `T` it expects; a message of another shape fails signature
verification. There is no "wrong operation type" error to forget to write.

### What it does not cover

`TYPE_DEF` is **shallow**. A nested `#[contract_type]` appears in it as
`ContractTypeView::Named(NAME)`, not as an inlined definition — transitive
definitions are resolved by spec-tool at link time, not in a const
(`ContractTypeRefSpec::INFO` is a `Named` ref, and contract-spec says so).

A nested change that alters the encoding still changes the message via the body
bytes. What slips through is a nested change that **preserves** the encoding:
renaming a field, or swapping two fields of the same type. That is a semantic
reversal an old signature will still verify against. Pinned in
`operation.rs::tests::nested_renames_are_not_caught_by_type_def`.

**EIP-712 does not have this gap** — its `encodeType` appends referenced struct
definitions transitively, sorted by name. Here the lever for a nested change is
the separator's version (§2). Read the two mechanisms together:

- **separator version** — the app's schema epoch; manual, retires everything.
- **`TYPE_DEF`** — distinguishes shapes within an epoch, and catches top-level
  drift for free.

Closing the gap properly would need either a build-time check that hashes the
spec-tool-resolved type closure and fails when it changes without a version
bump, or getting that closure into a const. Both are larger than this change.

### What can be signed

Any named `#[contract_type]` — struct, enum or union, including tuple structs
(the macro handles `Fields::Unnamed`). **Bare tuples and primitives cannot**:
they have a `ContractTypeRefSpec` but no `ContractTypeSpec`. That is deliberate.
An anonymous shape has no identity to bind, so signing one would be
indistinguishable from signing any other value with the same field types. Wrap
it in a named type.

## 5. Auth tokens: why these are "operations"

`SignedOp::execute_signed_op` returns `Vec<Signed<Opaque>>`, matching
`ERC1271::is_valid_signature`. A validated operation mints an auth-token *tree*:
`Signed<T>` fields encode to nothing and become child nodes addressed to their
own consumers, so **one signature can authorize a whole scoped call graph**.
Each downstream contract consumes exactly its own node; none can consume
another's. Callers destructure with `AssumeTokens::assume_tokens`.
`example-contracts/signer_minter` shows the shape.

`kAuthTokenMaxChildNodes` and `kAuthTokenMaxLen` bound the tree.

## 6. Known gap: contracts cannot read their network hash

There is no syscall for it, and `sdk::block` exposes only the height. It cannot
be recovered from the contract address either: pubkey account derivation is
`sha256(cxs(DomainSeparator("PubkeyAccount")) || cxs({pubkey, nonce}))`, with no
network in the preimage — the same pubkey yields the same address on every
network, which is exactly why transaction signatures carry a network hash
explicitly.

So `check_signed_op` takes the network hash as a parameter, and a verifying
contract must be **told** it at deployment and store it as configuration. Until
the protocol grows a way to read it, a contract that hardcodes or omits it is
replayable across any network carrying the same address.

A `GetNetworkHash` syscall would remove this footgun. That is a protocol change
and is the only part of this design that would touch `hogd`.

## 7. What ships where

| Crate | Contents |
|---|---|
| `groundhog-domain-separator` | `domain_separator!`, `SIGNED_OP`, `SignedOp<T>`, `DomainSeparator::preimage` |
| `groundhog-signed-op-if` | `SignedOp` trait, `Operation` wire type, `SignedOpError`, `check_signed_op::<T>`, `decode_body`, `signed_op_of` |
| `groundhog-test-vectors` | `signed-op` subcommand |

`groundhog-contract-util` is untouched: entrypoint IDs are unrelated to this.

## 8. The client half

`@groundhogchain/client` must reproduce §3 byte for byte, which means it needs
`cxs(TYPE_DEF)` for the payload type. TypeScript has the spec *model*
(`ContractTypeDef` from `@groundhogchain/contract-spec`) and the generated
bindings ship a full `spec: ContractSpec`, but there is **no spec serializer on
the TS side** — `.spec.xdr` is turned into TypeScript by the Rust `groundhog`
CLI, so nothing in the client ever encodes a spec.

Two ways to close that, in preference order:

1. **`groundhog codegen ts` emits the type-def bytes** per contract type, as a
   constant beside the spec. The value is already computed on the Rust side;
   this is where it should come from, and it keeps a hand-written encoder out of
   the client.
2. **A CXS encoder for the spec model in `client-ts`**, mirroring
   `contract-spec/src/view.rs` — 3 type-def kinds and 18 `ContractTypeView`
   variants, roughly 150 lines, pinned by `type_def_cxs_hex` in the vectors.

## 9. Follow-ups not done here

- The client half (§8).
- A worked example contract with nonce storage.
- `GetNetworkHash` (§6).
- Transitive type binding (§4).
- Bumping the `groundhog-test-vectors` pin in
  `client-ts/scripts/regen-test-vectors.sh` once a release carries `signed-op`.
