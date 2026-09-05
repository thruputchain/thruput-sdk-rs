# Signed operations: unordered nonces via `Uniquifier`

Status: implemented in this change. Supersedes the nonce model in
`2026-08-31-signed-operations.md`.
`groundhog-signed-op` (0.1.x line, one consumer: client-ts #44).

## Goal

Replace the sequential per-signer nonce with the chain's own replay model —
a signer-chosen `(nonce, block_expiry_height)` pair, consumed once, dead after
expiry — and drop `next_signed_op_nonce`. The interface is designed so that
the contract-storage cache can later be swapped for the chain's replay cache
(a `ConsumeUniquifier`-style syscall, `StorageKeyType::ReplayCache`) without
touching the trait, the wire type, the preimage, or the errors.

## Wire

```rust
#[sdk::contract_type]
pub struct Operation {
    pub signer: Ed25519PublicKey,
    /// Last height (inclusive) at which this operation may execute.
    pub block_expiry_height: BlockHeight,
    /// Chosen by the signer; any value, as long as the
    /// (nonce, block_expiry_height) pair is fresh at this contract.
    pub nonce: u64,
    pub body: BytesM,
}

impl Operation {
    pub fn uniquifier(&self) -> Uniquifier { .. }   // sdk::types::transaction::Uniquifier
}
```

The two fields sit in `Uniquifier`'s declared order (`block_expiry_height`,
then `nonce`), so `cxs(block_expiry_height) || cxs(nonce)` is byte-identical
to `cxs(Uniquifier { .. })` — CXS concatenates struct fields without framing.
Not nesting the foreign type is deliberate: `Uniquifier` has no
`ContractTypeSpec`, and a named type only survives spec shaking through the
anchor fn `#[derive(ContractTypeSpec)]` emits; giving a groundhog-xdr type one
means a derive in protocol-rs or a newtype plus hand-rolled anchor. That is a
follow-up if a named `Uniquifier` in the TS bindings is wanted; the bytes do
not change either way.

### Preimage (breaking)

```text
cxs(DomainSeparator("SignedOp")) || network || verifying_contract
    || cxs(T::TYPE_DEF) || cxs(block_expiry_height) || cxs(nonce) || cxs(body)
```

Today the tail is `cxs(nonce) || cxs(valid_until)`; the order flips and the
field is renamed. `groundhog_domain_separator::SignedOp` gains
`uniquifier: Uniquifier` in place of the two fields. Vectors regenerate
(`groundhog-test-vectors signed-op`); client-ts updates its preimage builder.

## Semantics — the chain's, restated

Each line mirrors the transaction replay cache (groundhog-xdr
`transaction::Uniquifier`, `storage_item::kDeltaTypeConsumeUniquifier`).

- **Identity of a consumption** is the pair `(nonce, block_expiry_height)`,
  scoped to the verifying contract. Same nonce with a different expiry is a
  different operation and does not conflict.
- **Scope is the contract, not the signer**, as the chain scopes by invoked
  address. All keys authorized at one wallet share the space, so a wallet
  with several signers has them draw nonces at random; a collision is then
  the signer's own doing, not an attack.
- **Validity**: `current_height <= block_expiry_height` (inclusive — "invalid
  after this block").
- **Same-block double consumption conflicts at commit**, as
  `kDeltaTypeConsumeUniquifier` "conflicts with any other attempt to consume
  the same Uniquifier". Contract-side that is the exclusive lock on the slot.
- **Refusal reverts** (unchanged): a refused operation consumes nothing.
- **No ordering.** A later operation does not retire an earlier one;
  cancellation is the expiry. An explicit `invalidate(Uniquifier)` — consume
  without executing — is a possible later addition, not in scope.
- **Garbage**: an entry is dead once `current_height > block_expiry_height`.
  Contract-side, `prune` deletes such an entry (anyone may; no-op if absent)
  and can never re-enable replay, because `Expired` is checked before the
  cache. Chain-side GC replaces this when the syscall lands.

## Interface (`groundhog-signed-op-if`)

```rust
#[contract(client = SignedOpClient, throws = SignedOpError)]
pub trait SignedOp {
    fn execute_signed_op(&mut self, op: Operation, signature: &Ed25519Signature)
        -> Signed<Opaque>;   // one root token; downstream grants are its children
}

#[sdk::contract_exception] #[non_exhaustive]
pub enum SignedOpError {
    InvalidSignature = 1,
    Expired          = 2,
    AlreadyConsumed  = 3,   // takes UnexpectedNonce's code
    MalformedBody    = 4,
    NotYetExpired    = 5,   // prune before the expiry has passed
}
```

`next_signed_op_nonce` is removed: nothing on-chain is needed to choose a
fresh pair. The return type is one token rather than a `Vec`, and the
interface crate drops `alloc` with it, so a no-alloc contract can call a
wallet and receive what it minted; only the verifying component allocates.

## Component (`groundhog-signed-op`)

```rust
/// Stateless: expiry, decode, signature.
pub fn verify_signed_op<T>(op: &Operation, signature: &Ed25519Signature) -> T;

#[sdk::storage]
pub struct SignedOpReplayCache { consumed: Map<Uniquifier, Value<()>> }

impl SignedOpReplayCache {
    /// Expiry → cache hit (AlreadyConsumed) → decode + signature → consume
    /// under an exclusive lock. Throws on any failure; writes nothing.
    pub fn consume<T>(&mut self, op: &Operation, signature: &Ed25519Signature) -> T;
    /// Delete a dead entry. Throws unless current_height > block_expiry_height.
    pub fn prune(&mut self, uniquifier: &Uniquifier);   // NotYetExpired until then
}
```

`Map<K, _>` takes any `WriteXDR` key, `()` is XDR, so this needs no new
storage machinery. When the chain exposes its replay cache to contracts, only
the body of `consume` changes.

## Impact

- `domain-separator/operation.rs` + layout test; `test-vectors/signed_op_vectors.rs`.
- `signed-op-if` (type, trait, errors); `signed-op` (component rename);
  `spec-tool` describable errors.
- `example-contracts/signed_op_wallet`: the smallest `SignedOp` wallet, whose
  tests pin the replay semantics above.
- The ERC20 permit change (#428) rebases onto this: its wallet moves to
  `SignedOpReplayCache` and its replay tests to the pair model.
- `groundhog-erc1271-if` is removed, with the `erc1271_*` examples; the
  `integration_test_contracts` submodule moves to a commit whose
  `simple_wallet` implements `SignedOp` instead.
- client-ts (separate repo): preimage builder order + field rename.

