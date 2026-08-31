# Signed Operations

**Status:** implemented in `groundhog-domain-separator`, `groundhog-contract-util`,
`groundhog-signed-op-if`, `groundhog-test-vectors`, and `@groundhogchain/client`.

**Audience:** anyone implementing the verifier side (the C++ validator, a third
party client) needs the byte layout in §3 and the vectors it is pinned to.

## 1. What this is

A *signed operation* is a structured message, signed off-chain, that authorizes
an on-chain action. The signer need not be the submitter, which is the pattern
behind permits, session keys, meta-transactions and delegated wallet actions.

Ethereum's answer to this is EIP-712, and the vocabulary transfers even though
none of the bytes do. EIP-712 is three things at once, and Groundhog already had
two of them:

| EIP-712 | Groundhog | Before this change |
|---|---|---|
| `keccak256` | `sha256` (syscall + host backends) | ✅ |
| ABI encoding | XDR / CXS | ✅ |
| `\x19\x01` envelope | CXS domain-separator prefix | ✅ |
| `EIP712Domain.name` | `DomainSeparator.domain_separator` | ✅ |
| `EIP712Domain.version` | — | ❌ added (§2) |
| `EIP712Domain.chainId` | `NetworkHash` | ✅ for transactions only |
| `EIP712Domain.verifyingContract` | — | ❌ added (§3) |
| `typeHash` / `encodeType` | — | ❌ added (§4) |
| nonce, deadline | `Uniquifier` | ✅ for transactions only |
| ERC-1271 `isValidSignature` | `groundhog-erc1271-if` | ✅ |
| `eth_signTypedData_v4` | — | ❌ added (`@groundhogchain/client/operations`) |

The name "EIP-712" is deliberately **not** used. Groundhog is
sha256/ed25519/XDR where EIP-712 is keccak256/secp256k1/ABI; borrowing the
number would promise a byte- and signer-level compatibility that does not
exist. ERC-1271 keeps its name because it is an interface shape
(`(message, signature) -> validity`), not a crypto binding.

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
transaction signature. `crates/domain-separator/src/separator.rs` keeps a
mirror of the protocol type plus a drift guard
(`prefix_matches_real_domain_separator`) that fails if the two diverge.

`::` is the same delimiter, for the same reason, as in entrypoint IDs and event
signatures: without it `("MyApp1", "2")` and `("MyApp", "12")` share a signing
domain.

Bumping the version retires every signature made under the old schema at once.
Bump it whenever a signed message's shape changes — adding a field to a struct
does not change its name, and CXS has no framing to notice with.

## 3. The preimage

```text
sha256 is NOT applied before signing. These bytes are signed directly.

  cxs(DomainSeparator("SignedOp::1"))   13 bytes: 00 0b "SignedOp::1"
  network                               32 bytes, verbatim
  verifying_contract                    32 bytes, verbatim
  type_id                                8 bytes, verbatim
  nonce                                 CXS varint (u64), 1–9 bytes
  valid_until                           CXS varint (i64), 1–9 bytes
  body                                  CXS var-opaque: length prefix + bytes
```

Worked example (`specs/test-vectors/signed-op.json`, vector 2):

```text
000b5369676e65644f703a3a31          separator
5a5a…5a (32)                        network
0303…03 (32)                        verifying_contract
d740e1baead3149d                    type_id
01                                  nonce = 1
7fe803                              valid_until = 1000
012a                                body = [0x2a], length-prefixed
```

Notes for implementers:

- **The first 72 bytes after the separator are fixed-width**; the two integers
  after them are CXS varints, which are self-delimiting, so the body's start is
  unambiguous. Do not assume 8-byte integers — `nonce = 1` is one byte.
- **The body is length-prefixed.** It is `&[u8]`, not a generic `T`, and this is
  load-bearing: a typed `T` would inline its fields, so the same logical body
  would hash two different ways depending on which side held it as bytes.
  A signer with a struct encodes it first.
- **Varint canonicality is not a concern** because the preimage is only ever
  *constructed*, never parsed. Both sides build it from typed fields and
  re-encode. A verifier that instead accepted preimage bytes over the wire would
  have to reject non-canonical varints itself.
- **No pre-hash.** Ed25519 hashes its own message, so signing a digest would add
  an on-chain SHA-256 and a second convention for no cryptographic gain. This
  matches `transaction_signature_preimage`. `SignedOp::hash()` exists as a
  stable *identifier* (for nonce records, logs, dedup) and is what the vectors
  compare, but it is not what gets signed.

Each header field closes exactly one replay path, and none is optional:

| Field | Without it, the signature replays |
|---|---|
| `network` | on another network carrying the same address |
| `verifying_contract` | against a different contract |
| `type_id` | as a different kind of message |
| `nonce` / `valid_until` | again, or forever |

## 4. Operation type IDs

```text
type_id = sha256("Interface::MajorVersion::operation")[0..8]
```

This is byte-for-byte the entrypoint-ID derivation, on a literally shared code
path (`selector_from_triple` in `groundhog-contract-util`, `selectorFromTriple`
in `client-ts`), so the two cannot drift. An operation named after the method it
authorizes gets that method's ID; the two are never compared and never share a
position, so the coincidence is a convenience, not a constraint.

Contracts should embed the result as an `OperationTypeId::from_u64` literal
rather than deriving it at runtime, exactly as generated entrypoint IDs do.

### Why a type ID is necessary

CXS concatenates struct fields with no framing — no type tag, no field count, no
length prefix. `Grant { amount: u64 }` and `Revoke { id: u64 }` encode
identically, so without a type binding a signature over one *is* a signature
over the other.

This bites hardest with auth tokens, where `Signed<T>` fields encode to **zero
bytes** (`signer.rs`): two authorizations differing only in which contract their
children are addressed to have byte-identical payloads. Node-level safety comes
from `AuthTokenNode.consumer_address`, but that is enforced at consume time, not
at signature time.

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

A `GetNetworkHash` syscall would remove this footgun and let
`check_signed_op` read it directly. That is a protocol change and is not part
of this work.

## 7. What ships where

| Crate / package | Contents |
|---|---|
| `groundhog-domain-separator` | `domain_separator!`, `SIGNED_OP`, `SignedOp`, `OperationTypeId`, `DomainSeparator::preimage` |
| `groundhog-contract-util` | `operation_type_id`, shared `selector_from_triple` |
| `groundhog-signed-op-if` | `SignedOp` trait, `Operation` wire type, `SignedOpError`, `check_signed_op`, `signed_op_preimage` |
| `groundhog-test-vectors` | `signed-op` subcommand |
| `@groundhogchain/client/operations` | `signOperation`, `signedOperationPreimage`, `operationTypeId`, `toWireOperation` |

The two implementations are pinned together by
`specs/test-vectors/signed-op.json`, replayed in
`src/operations/crossSdk.test.ts`. Each vector carries every intermediate stage
(separator prefix, type ID, struct encoding, signed message, identifier) so a
failure names the stage that drifted.

## 8. Follow-ups not done here

- A worked example contract (`example-contracts/signed_op_wallet`) implementing
  the trait with nonce storage.
- `GetNetworkHash` syscall (§6).
- Rendering an operation for display from its contract spec. The spec already
  carries named fields and types (`ContractStructDefView`), which is the raw
  material EIP-712 wallets use for their structured-message UI; nothing consumes
  it for that yet.
- Bumping the `groundhog-test-vectors` pin in
  `client-ts/scripts/regen-test-vectors.sh` once a release carries `signed-op`.
