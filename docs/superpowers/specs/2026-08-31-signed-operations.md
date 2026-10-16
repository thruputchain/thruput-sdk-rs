# Signed Operations

> **Amended 2026-09-05.** The sequential per-signer nonce and the
> `next_signed_op_nonce` view described below were replaced by the chain's
> unordered `(nonce, block_expiry_height)` uniquifier model; see
> `2026-09-05-signed-op-uniquifier.md`. The preimage now carries a
> `Uniquifier` (expiry first, then nonce) where this note has nonce then
> `valid_until`. `Operation` no longer carries `signer`: the verifier takes
> the key from the contract's own state. Everything else here stands.

**Status:** implemented in `groundhog-domain-separator`, `groundhog-signed-op-if`,
`groundhog-signed-op` and `groundhog-test-vectors` (`signed-op` vectors). The TypeScript client half
is not done.

A *signed operation* is a structured message, signed off-chain, that
authorizes an on-chain action. The signer need not be the submitter, which is
the pattern behind permits, session keys and delegated wallet actions. It is
Groundhog's analogue of EIP-712's typed-data signing; the name is not borrowed
because none of the bytes are (sha256/ed25519/CXS, not keccak/secp256k1/ABI).

`hogd` needs no changes. A contract verifies with the `VerifySignature` syscall
and mints auth tokens with `MintAuthToken`; the node never parses an operation.

## The signed bytes

```text
cxs(DomainSeparator("SignedOp"))   10 bytes: 00 08 "SignedOp"
network                            32 bytes, verbatim (GetNetworkHash)
verifying_contract                 32 bytes, verbatim (GetCurrentAddress)
cxs(T::TYPE_DEF)                   the payload type's spec definition
nonce                              CXS varint (u64)
valid_until                        CXS varint (i64)
cxs(body)                          the payload value, fields inlined
```

Signed directly with no pre-hash, like `transaction_signature_preimage`.
Everything after the first 74 bytes is variable-length but self-delimiting; a
client must not assume fixed-width integers.

Each field closes one replay path: `network` (another network with the same
address), `verifying_contract` (another contract), `nonce`/`valid_until` (the
same contract, again or forever), `T::TYPE_DEF` (the same bytes read as a
different kind of message). The last exists because CXS has no framing:
`Grant { amount: u64 }` and `Revoke { id: u64 }` encode identically, and a
`Signed<_>` auth-token field encodes to nothing. `#[contract_type]` already
emits `ContractTypeSpec::TYPE_DEF` as a const, so the preimage includes the
type's own definition and adding or renaming a field retires old signatures.

`TYPE_DEF` is shallow (nested types appear by name). That is fine here: a
contract address is bound to one code image for its life, so
`verifying_contract` already pins every nested type's definition.

## Wire form and verification

`groundhog_signed_op_if::Operation` carries `signer`, `nonce`, `valid_until`
and the CXS `body`. The interface crate is declaration-only (trait, wire type,
error), so a caller depends on it without pulling in verification. The network
and verifying contract are deliberately not on the wire; the verifier reads
them from syscalls.

`groundhog_signed_op::SignedOpNonces` is a per-signer sequence number (`Map<Ed25519PublicKey,
Value<u64>>`). `verify_and_spend::<T>` checks the nonce is the signer's next,
checks expiry, decodes the body as `T`, rebuilds the preimage from the decoded
value, verifies the signature, then increments. Decoding first is what binds
`T::TYPE_DEF` and canonicalises the body. Sequential nonces mean a signer's
operations execute in signing order and any later operation retires an
unexecuted earlier one; that is the same model as EIP-2612 permits, and it is
enough until a contract needs something else.

## The client half

`@groundhogchain/client` has to reproduce the bytes above, including
`cxs(TYPE_DEF)` for the payload type. Nothing on the TS side encodes a spec
today; the cleanest route is for `groundhog codegen ts` to emit the type-def
bytes per contract type beside the generated spec. The `signed-op` vectors pin
every intermediate stage (`type_def_cxs_hex`, `body_cxs_hex`,
`signed_message_hex`) so a client that drifts fails at the stage that drifted.
