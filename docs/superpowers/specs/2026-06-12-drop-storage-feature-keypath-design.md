# Drop the `storage` feature; alloc-independent Keypath — design

**Date:** 2026-06-12
**Branch:** lands in PR #240 (continuation of the macros-cluster work).
**Constraint (Geoff):** the `alloc` feature gating stays exactly as-is — it
must remain possible for users to ship their own allocators.

## Motivation

`#[contract]` now always loads state through the storage path, which forced
deliberately storage-free contracts (erc20_no_storage) to enable the sdk
`storage` feature. The feature split is historical: it dates from when every
storage-tree node was eagerly loaded as a bytestring; nodes are virtual now.
Priorities (Geoff): codesize, end-user cognitive load, minimal hashing, and
the `alloc` feature stays exactly as it is (including the ability for users
to ship their own allocators).

Measurements (signer_leaf, optimized wasm, no storage/alloc use):
featureless 4914 B; `+alloc` 4914 B; `+storage` 5364 B. The +450 B is the
`LibraryExn::Storage(StorageError)` variant dragging the nested StorageError
codec into the always-linked abort path — not the storage types (symbol sets
identical). NOTE: single-contract numbers are indicative only; verification
below uses the whole corpus as a hard gate.

## Decisions

1. Delete the `storage` Cargo feature — the storage module is always
   compiled. The `alloc` feature is untouched.
2. Make `Keypath` work without alloc via a caller-visible buffer choice
   (below), since the always-on `#[contract]` state path depends on it.
3. Flatten `StorageError` into `LibraryExn` (reclaims the ~450 B; fixes the
   storage half of the feature-dependent wire-format footgun).
4. Bare-impl contracts drop gratuitous `self` receivers instead of wearing
   `#[storage]`; only trait impls keep `#[storage]` unit structs.
5. Rename `erc20_no_storage` → `erc20_raw_storage`.

## 1. Keypath redesign (append-buffer, alloc-independent)

Representation stays "append component encodings, hash once at the end" —
hashing count is unchanged from today (one sha256 per derived key, memoized).
The Vec is replaced by a buffer abstraction selected by the existing `alloc`
feature:

```rust
/// Append-only byte buffer backing a Keypath preimage.
pub trait KeyBuf: Default + Clone {
    /// Append `bytes`; throws (typed, see below) on overflow in bounded impls.
    fn append(&mut self, bytes: &[u8]);
    fn as_slice(&self) -> &[u8];
}

pub struct StaticKeyBuf<const N: usize> { buf: [u8; N], len: u16 }
// impl KeyBuf for StaticKeyBuf<N>            — always available
// #[cfg(feature = "alloc")] impl KeyBuf for alloc::vec::Vec<u8>

#[cfg(feature = "alloc")]
pub(crate) type KeyBufImpl = alloc::vec::Vec<u8>;
#[cfg(not(feature = "alloc"))]
pub(crate) type KeyBufImpl = StaticKeyBuf<256>;

#[derive(Clone, Debug)]
pub struct Keypath { buf: KeyBufImpl, exclusive: bool }
```

- **Same keys in both modes.** The appended bytes are identical regardless
  of buffer impl; the derived key is `sha256(buf)`. The feature changes only
  memory behavior and the failure mode, never the keyspace.
- **API:** the mutation API is replaced by child derivation:
  - `Keypath::root()`
  - `child_index(&self, i: u64) -> Keypath` — appends `[0x01] ‖ encode(i)`
  - `child_item<T: WriteXDR + ?Sized>(&self, t: &T) -> Keypath` — appends
    `[0x00] ‖ u16-len ‖ encode(t)` (length framing)
  - `generate_key(&self) -> StorageKey` — `sha256(buf)`
  - `is_exclusive`/`set_exclusive` unchanged.
  `push_item`/`push_index`/`push_bytes`/`seq_begin`/`seq_next`/`seq_end`,
  `flattened()`, and `KeypathComponent` are deleted (zero external users;
  the seq API existed only to serve the mutable-Vec representation — this
  also removes the stale `leave_seq()` panic-message finding).
  `derive(Storage)` codegen switches from the seq dance to
  `keypath.child_index(i)` per field. `Map::make_entry_keypath` and
  `Vector::make_elem_keypath` switch to `child_item`. With framing + tag
  bytes, the boundary-shift and Index-vs-u64-item collision classes are
  both structurally closed.
- **Every storage key changes** (framing bytes alter preimages). Pre-launch,
  acceptable; nothing outside keypath.rs reads preimage bytes.
- **Overflow (static mode only):** throws the new flat
  `LibraryExn::StorageKeypathOverflow` (see §2) — not a bare panic — with
  rustdoc on `Keypath` documenting the 256-byte no-alloc budget and the
  outs (enable `alloc`, or pre-hash oversized keys).
- `KeyCache` stays; `value.rs`'s load path is fixed to use the cached key
  (closing the "Value bypasses its own KeyCache" finding). NNInt gains a
  `KeyCache` the same way only if trivially mechanical — otherwise leave
  for the storage cluster.
- Public `Keypath` rustdoc documents the scheme (framed components, one
  hash, no-alloc budget) — addressing the keypath-docs finding in passing.

## 2. StorageError flattening

- Delete `StorageError`, its `ContractTypeSpec`/`ContractTypeName`/codec,
  `From<StorageError> for LibraryExn`, and `storage_syserr` (which
  hardcoded a sometimes-wrong `kBytestringGet` callno).
- `LibraryExn` (always one fixed layout now):
  ```rust
  StorageSerialization = 8,
  StorageIndexOutOfBounds { len: u64, access: u64 } = 9,
  StorageKeypathOverflow = 10,
  ```
  with debugging Display strings of equivalent fidelity.
- Former `StorageError::Sys` paths throw `LibraryExn::Syscall(SyscallExn)`
  directly with the accurate callno. `oob_throw`/`throw_load_exn`/
  `throw_store_exn` retarget. `is_storage_key_not_exist` logic moves to its
  remaining callers' level (`SyscallExn`).

## 3. Feature removal mechanics

- `crates/sdk/Cargo.toml`: delete the `storage` feature; fold
  `groundhog-macros/storage` into the unconditional macros dep. Any feature
  lists naming `storage` drop it. `[[test]] required-features = ["storage"]`
  entries: replace with whatever the tests actually still need (likely
  `std`); do not preserve a dead feature for the harness.
  NOTE: today `storage = ["alloc", ...]` — storage pulled in alloc. After
  this change, no-alloc + storage must build and pass tests (that's the
  point of §1).
- `crates/macros`: delete its `storage` feature and the cfg gates on the
  `Storage`/`Deletable` derives and `#[storage]` attr.
- `crates/sdk/src/**`: remove `#[cfg(feature = "storage")]` everywhere.
  `#[cfg(feature = "alloc")]` gates REMAIN where they gate genuinely
  alloc-needing APIs — in storage that's the owned-return APIs
  (`Bytestring::get`/`try_get` returning `Box<[u8]>`, RCQ `*_raw` methods)
  which become per-method `#[cfg(feature = "alloc")]`.
- Workspace sweep: every `sdk = { ..., features = ["storage", ...] }`
  drops the entry (keep other features); `scripts/dev.sh` `-Fstorage,std` →
  `-Fstd` (or what the suite actually needs); `cargo doc --features=dev,storage`
  drops storage; CI workflow strings unchanged (they call dev.sh).
- Trybuild: `compile-fail-storage/` merges into `compile-fail/` if the
  harness split existed only for the feature; otherwise leave layout.

## 4. Contract sweep

- Bare-impl contracts that gained `#[storage]`/`#[sdk::storage]` unit
  structs in the earlier migration drop their `self` receivers (becoming
  associated functions) and lose the attribute and any
  `features = ["storage"]` entries. Entry-point-ID shifts from signature
  changes are acceptable pre-launch; tests verify behavior.
- Trait impls keep trait-mandated receivers + `#[storage]` unit structs
  (now featureless): erc20_raw_storage, itc_erc20, erc1271_wallet.
- A no-alloc contract with `self` methods must now actually work: add or
  adapt one integration/example contract to build with NO sdk features and
  a `#[storage]` unit struct, proving the §1 design end-to-end.

## 5. Rename erc20_no_storage → erc20_raw_storage

Directory, package name, workspace members, all references (erc20-test
wiring, bottle-test deps, dev.sh/check-wasm-size allowlists, docs/README).
Module doc explains it manages balances via raw storage keys/syscalls.

## 6. Verification — codesize and behavior are HARD GATES

1. **Baseline first:** before changes, build the full corpus exactly as
   `scripts/dev.sh` does (`build-examples`, `build-itc`) and record all
   `target/optimized/*.wasm` sizes.
2. **After:** rebuild identically; diff per contract. No contract grows
   > ~100 B without an identified, explicitly-accepted cause. If always-on
   storage measurably bloats storage-free contracts beyond the reclaimed
   LibraryExn bytes, STOP and bring options back to Geoff.
3. `scripts/check-wasm-size.sh` passes.
4. Full suites pass: sdk (with the new feature matrix: default, alloc,
   std combinations that CI covers), macros, examples, integration check,
   including at least one no-alloc no-feature contract with self methods.
5. Greps clean: no `feature = "storage"` under crates/; no
   `features = ["storage"]` sdk-dep entries anywhere.
6. CI green on PR #240; greptile re-review addressed.
