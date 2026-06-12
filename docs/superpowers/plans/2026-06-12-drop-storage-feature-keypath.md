# Drop Storage Feature + Alloc-Independent Keypath Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete the sdk `storage` Cargo feature (storage always compiled, working with or without `alloc`), flatten `StorageError` into `LibraryExn`, rewrite `Keypath` on a feature-selected append buffer, drop gratuitous `self` receivers from test contracts, and rename `erc20_no_storage` → `erc20_raw_storage` — per `docs/superpowers/specs/2026-06-12-drop-storage-feature-keypath-design.md`.

**Architecture:** `Keypath` keeps "append framed component encodings, hash once, memoize" semantics on a `KeyBuf` abstraction (heap `Vec` under `alloc`, `StaticKeyBuf<256>` otherwise — identical keys either way). The mutation API (`seq_*`, `push_*`) is replaced by `child_index`/`child_item`. `StorageError` becomes three flat `LibraryExn` variants. The `alloc` feature itself is untouched (custom allocators stay supported).

**Tech Stack:** Rust nightly-2026-01-30; canonical sdk test command after this work: `cargo test -p sdk -Fstd --tests` (storage flag gone). Branch: `geoff/macros-cluster-fixes` (PR #240). `integration_test/contracts` is a git submodule on its own `geoff/macros-cluster-fixes` branch — commit there separately and bump the gitlink.

**Hard gates (from spec §6):** corpus codesize diff (Task 0 baseline vs Task 6) with no unexplained growth > ~100 B per contract; `cargo check -p sdk` (featureless = no-alloc storage) must pass after Task 3.

---

### Task 0: Corpus size baseline (BEFORE any code change)

- [ ] **Step 1: Build everything and record sizes**

```bash
cd /Users/geofframseyer/groundhog/sdk_rs
./scripts/dev.sh build-examples && ./scripts/dev.sh build-itc
ls -l target/optimized/*.wasm | tr -s ' ' | cut -d' ' -f5,9 | sort -k2 > /tmp/wasm_sizes_baseline.txt
wc -l /tmp/wasm_sizes_baseline.txt
```

Expected: both build commands succeed; the manifest lists every example + enabled integration contract. If `build-itc` needs the hogd FFI and fails for infra reasons, note it and baseline examples only.

- [ ] **Step 2: No commit** (nothing changed); just confirm `/tmp/wasm_sizes_baseline.txt` exists and report its line count.

---

### Task 1: Flatten StorageError into LibraryExn

**Files:**
- Modify: `crates/sdk/src/exn/library.rs` (variant list ~lines 22-53, `From<StorageError>` impl ~73-78, `use crate::storage::*` gate at 5-6)
- Modify: `crates/sdk/src/storage/error.rs` (delete the type; keep retargeted helpers)
- Modify: `crates/sdk/src/storage/value.rs:45-67` (try_get/load_value)
- Check/Modify: anything matching `grep -rn "StorageError" crates --include="*.rs" | grep -v target`

- [ ] **Step 1: Replace the gated variant with flat variants**

In `crates/sdk/src/exn/library.rs`: delete `#[cfg(feature = "storage")] use crate::storage::*;`, the `Storage(StorageError) = 8` variant, and the `From<StorageError>` impl. Add in their place:

```rust
    #[cfg_attr(
        feature = "debugging",
        error("storage value failed to (de)serialize")
    )]
    StorageSerialization = 8,

    #[cfg_attr(
        feature = "debugging",
        error("Vector index out of bounds: len={len}, access={access}")
    )]
    StorageIndexOutOfBounds { len: u64, access: u64 } = 9,

    #[cfg_attr(
        feature = "debugging",
        error("storage keypath exceeds the no-alloc budget; enable the \
               `alloc` feature or pre-hash oversized keys")
    )]
    StorageKeypathOverflow = 10,
```

- [ ] **Step 2: Rewrite `crates/sdk/src/storage/error.rs`**

Delete `StorageError`, its derives/`ContractTypeName`, `storage_syserr`, and `is_storage_key_not_exist`. Keep the file as the home of the throw helpers:

```rust
use crate::exn::*;
use crate::xdr::*;

pub fn oob_throw(len: u64, access: u64) -> ! {
    library_throw(LibraryExn::StorageIndexOutOfBounds { len, access })
}

pub fn throw_load_exn(_r: ReadError) -> ! {
    library_throw(LibraryExn::StorageSerialization)
}

pub fn throw_store_exn(_: WriteError) -> ! {
    library_throw(LibraryExn::StorageSerialization)
}
```

(Check `library_throw`'s signature — it may take `impl Into<LibraryExn>`; adapt the call shape, not the semantics.)

- [ ] **Step 3: Rewrite Value's load path (and fix the KeyCache bypass)**

In `crates/sdk/src/storage/value.rs`, replace `try_get`/`load_value` (currently `let key = &self.key.keypath().generate_key();` — re-hashing every read) with:

```rust
    /// Deserialize and return the stored value, or None if the key doesn't exist.
    #[cfg_attr(feature = "debugging", track_caller)]
    pub fn try_get(&self) -> Option<T> {
        use crate::sys_result::bytestring::bytestring_get;

        let key = self.key.get(); // cached: hash computed once per handle
        match bytestring_get(key) {
            Ok(buf) => Some(xdrs::cxs::decode(&buf).unwrap_or_else(|_| {
                crate::exn::library_throw(
                    crate::exn::LibraryExn::StorageSerialization,
                )
            })),
            Err(e) if e.is_storage_key_not_exist() => None,
            Err(e) => crate::exn::library_throw(
                crate::exn::LibraryExn::Syscall(e),
            ),
        }
    }
```

Check the error type `sys_result::bytestring::bytestring_get` actually returns (it has its own `is_storage_key_not_exist` usage at `crates/sdk/src/sys_result/bytestring.rs:25,131` — mirror whatever that path does) and whether it is already a `SyscallExn` or needs `SyscallExn::new(SyscallNo::kBytestringGet, e)`. Delete `load_value` if nothing else calls it; update `get()` to share the same path.

- [ ] **Step 4: Sweep remaining StorageError references**

`grep -rn "StorageError" crates --include="*.rs" | grep -v target` — update every hit (tests asserting `LibraryExn::Storage(...)` codes, e.g. `root_error_codes.rs` if present; spec-tool/native-exec mentions). Selector/discriminant assertions move to the new flat variants.

- [ ] **Step 5: Verify**

Run: `cargo test -p sdk -Fstorage,std --tests && cargo test -p groundhog-macros`
Expected: PASS (the storage feature still exists in this task; it dies in Task 3).

- [ ] **Step 6: Commit**

```bash
git add crates
git commit -m "Flatten StorageError into LibraryExn

Deletes the nested exception-in-exception wrapping (and the
storage_syserr helper that hardcoded a sometimes-wrong callno).
LibraryExn now has one fixed wire layout regardless of features,
and the nested StorageError codec leaves every contract's abort
path (~450B of wasm in measurements)."
```

---

### Task 2: Keypath rewrite (append-buffer, child API)

**Files:**
- Rewrite: `crates/sdk/src/storage/keypath.rs`
- Modify: `crates/macros/src/storage.rs:85-130` (derive codegen)
- Modify: `crates/sdk/src/storage/vector.rs:25-40,142-146`
- Modify: `crates/sdk/src/storage/map.rs:114-121`
- Modify: `crates/sdk/tests/storage.rs:275-310` (manual layout reconstruction)
- Tests: new module in keypath.rs

- [ ] **Step 1: Write the new keypath.rs**

Replace the whole file (keep the `KeyCache` module at the bottom unchanged):

```rust
use crate::crypto::sha256;
use crate::types::StorageKey;
use core::fmt::Debug;
use xdrs::WriteXDR;

/// Append-only byte buffer backing a [`Keypath`] preimage.
///
/// Two implementations exist: a growing heap `Vec<u8>` (with the `alloc`
/// feature) and the fixed [`StaticKeyBuf`] fallback (without). Both append
/// identical bytes, so derived storage keys never depend on the feature —
/// only memory behavior and the overflow failure mode differ.
pub trait KeyBuf: Default + Clone + Debug {
    /// Append `bytes`, throwing `LibraryExn::StorageKeypathOverflow` if the
    /// buffer cannot hold them.
    fn append(&mut self, bytes: &[u8]);
    fn as_slice(&self) -> &[u8];
}

/// Fixed-capacity [`KeyBuf`] used when the `alloc` feature is disabled.
#[derive(Clone, Debug)]
pub struct StaticKeyBuf<const N: usize> {
    buf: [u8; N],
    len: u16,
}

impl<const N: usize> Default for StaticKeyBuf<N> {
    fn default() -> Self {
        Self {
            buf: [0; N],
            len: 0,
        }
    }
}

impl<const N: usize> KeyBuf for StaticKeyBuf<N> {
    fn append(&mut self, bytes: &[u8]) {
        let start = self.len as usize;
        let end = start.saturating_add(bytes.len());
        if end > N {
            crate::exn::library_throw(
                crate::exn::LibraryExn::StorageKeypathOverflow,
            );
        }
        self.buf[start..end].copy_from_slice(bytes);
        self.len = end as u16;
    }

    fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len as usize]
    }
}

#[cfg(feature = "alloc")]
impl KeyBuf for alloc::vec::Vec<u8> {
    fn append(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }

    fn as_slice(&self) -> &[u8] {
        self
    }
}

/// No-alloc keypath preimage budget (total framed bytes across all levels).
pub const STATIC_KEYPATH_CAPACITY: usize = 256;

#[cfg(feature = "alloc")]
type KeyBufImpl = alloc::vec::Vec<u8>;
#[cfg(not(feature = "alloc"))]
type KeyBufImpl = StaticKeyBuf<STATIC_KEYPATH_CAPACITY>;

const TAG_ITEM: u8 = 0x00;
const TAG_INDEX: u8 = 0x01;

/// Path identifying a storage slot.
///
/// A keypath is a sequence of framed components appended to a byte buffer:
/// `child_index(i)` appends `[0x01] ‖ i.to_be_bytes()` (SDK-assigned
/// structural positions: struct fields, Vector len/values regions) and
/// `child_item(t)` appends `[0x00] ‖ u16-len ‖ encode(t)` (user keys, e.g.
/// Map entries). The storage key is `sha256` of the buffer, computed once
/// and memoized by [`KeyCache`]. Framing makes distinct component sequences
/// produce distinct preimages.
///
/// Without the `alloc` feature, the preimage shares a fixed
/// [`STATIC_KEYPATH_CAPACITY`]-byte budget; exceeding it throws
/// `LibraryExn::StorageKeypathOverflow` (enable `alloc`, or pre-hash
/// oversized keys). A single `child_item` encoding is additionally capped
/// at the SDK's fixed encode-buffer size in both modes.
#[derive(Clone, Debug, Default)]
pub struct Keypath {
    buf: KeyBufImpl,
    exclusive: bool,
}

impl Keypath {
    pub fn root() -> Self {
        Self::default()
    }

    /// Child at an SDK-assigned structural position.
    pub fn child_index(&self, i: u64) -> Keypath {
        let mut child = self.clone();
        child.buf.append(&[TAG_INDEX]);
        child.buf.append(&i.to_be_bytes());
        child
    }

    /// Child keyed by an encodable value (e.g. a Map key).
    pub fn child_item<I: WriteXDR + ?Sized>(&self, item: &I) -> Keypath {
        let encoded = crate::internal::encode_xdr(item).unwrap_or_else(|_| {
            crate::exn::library_throw(
                crate::exn::LibraryExn::StorageSerialization,
            )
        });
        let bytes = encoded.as_slice();
        let len: u16 = bytes.len().try_into().unwrap_or_else(|_| {
            crate::exn::library_throw(
                crate::exn::LibraryExn::StorageKeypathOverflow,
            )
        });
        let mut child = self.clone();
        child.buf.append(&[TAG_ITEM]);
        child.buf.append(&len.to_be_bytes());
        child.buf.append(bytes);
        child
    }

    pub fn generate_key(&self) -> StorageKey {
        sha256(self.buf.as_slice()).into()
    }

    pub fn is_exclusive(&self) -> bool {
        self.exclusive
    }

    pub fn set_exclusive(&mut self) {
        self.exclusive = true;
    }
}
```

Notes for the implementer: `crate::internal::encode_xdr` is the featureless fixed-buffer encoder (`crates/sdk/src/internal/serialize.rs`, `MUBuf<ENCODE_MAX>`); confirm its visibility from the storage module (adjust `pub(crate)` if needed). Check `library_throw`'s exact signature and adapt. Keep the existing `exclusive_flag_propagates_through_clone` test, ported to `child_item`.

- [ ] **Step 2: Add keypath unit tests (same file)**

```rust
#[cfg(test)]
mod test_keypath {
    use super::*;

    #[test]
    fn exclusive_flag_propagates_through_clone() {
        let mut kp = Keypath::root().child_item("some bytes");
        kp.set_exclusive();
        assert!(kp.clone().is_exclusive());
    }

    #[test]
    fn distinct_paths_distinct_keys() {
        let root = Keypath::root();
        // boundary-shift case: [AA BB]+[CC] vs [AA]+[BB CC]
        let a = root.child_item(&[0xAAu8, 0xBB]).child_item(&[0xCCu8]);
        let b = root.child_item(&[0xAAu8]).child_item(&[0xBBu8, 0xCC]);
        assert_ne!(a.generate_key(), b.generate_key());
        // index vs same-value u64 item
        let c = root.child_index(0);
        let d = root.child_item(&0u64);
        assert_ne!(c.generate_key(), d.generate_key());
    }

    #[test]
    fn derivation_is_deterministic() {
        let k1 = Keypath::root().child_index(3).child_item(&42u64);
        let k2 = Keypath::root().child_index(3).child_item(&42u64);
        assert_eq!(k1.generate_key(), k2.generate_key());
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn static_and_vec_bufs_agree() {
        fn fill<B: KeyBuf>(buf: &mut B) {
            buf.append(&[1, 2, 3]);
            buf.append(&[4]);
        }
        let mut s = StaticKeyBuf::<64>::default();
        let mut v = alloc::vec::Vec::<u8>::default();
        fill(&mut s);
        fill(&mut v);
        assert_eq!(s.as_slice(), v.as_slice());
    }
}
```

(Both impls must produce identical bytes for identical appends — that's what guarantees keys don't depend on the `alloc` feature.)

- [ ] **Step 3: Update the derive codegen**

`crates/macros/src/storage.rs`, named-fields branch: replace the seq dance:

```rust
    for (i, field) in named_fields.iter().enumerate() {
        let field_name = &field.ident;
        let field_type = &field.ty;
        field_names.extend(quote! { #field_name, });

        let idx = i as u64;
        field_initializers.extend(quote! {
            let #field_name = keypath.child_index(#idx).into();
        });

        generics
            .make_where_clause()
            .predicates
            .push(syn::parse_quote!(#field_type: From<Keypath>));
    }
```

and the generated `from` body becomes (no `mut`, no `seq_begin`, no stale comment):

```rust
                fn from(keypath: Keypath) -> Self {
                    #field_initializers

                    Self {
                        #field_names
                    }
                }
```

- [ ] **Step 4: Update Vector and Map**

`crates/sdk/src/storage/vector.rs`:

```rust
impl<S> From<Keypath> for Vector<S> {
    fn from(keypath: Keypath) -> Self {
        let len_kp = keypath.child_index(0);
        let values_kp = keypath.child_index(1);

        Self {
            keypath,
            len: Value::from(len_kp),
            values: Map::from(values_kp),
        }
    }
}
```

and `make_elem_keypath`:

```rust
    fn make_elem_keypath(&self, index: u32) -> Keypath {
        self.values.keypath().child_item(&index)
    }
}
```

`crates/sdk/src/storage/map.rs`:

```rust
    fn make_entry_keypath(&self, entry_key: &K) -> Keypath
    where
        K: WriteXDR,
    {
        self.keypath.child_item(entry_key)
    }
```

- [ ] **Step 5: Update the manual layout test**

`crates/sdk/tests/storage.rs:275-310` reconstructs Vector's layout with `seq_begin`/`seq_next`/`push_item` — rewrite using `child_index(1)`, `child_index(0)`, `child_item(&idx)` to assert the same equivalences against the real Vector handles. Grep the whole tests dir for other `seq_`/`push_item` users and port identically.

- [ ] **Step 6: Verify**

Run: `cargo test -p sdk -Fstorage,std --tests && cargo test -p groundhog-macros && ./scripts/dev.sh test-examples`
Expected: PASS. Every storage key changes value — failures must only be in tests that pinned literal key bytes (update them); behavioral tests must pass unmodified.

- [ ] **Step 7: Commit**

```bash
git add crates
git commit -m "Rewrite Keypath: framed append-buffer with child_index/child_item

Replaces the Vec-of-components representation and the seq_begin/
seq_next/seq_end mutation API. The buffer is a KeyBuf (heap Vec with
alloc; StaticKeyBuf<256> without — identical keys either way). Framing
(tag + length) closes the boundary-shift and index-vs-item collision
classes. All storage keys change (pre-launch). Hashing count is
unchanged: one sha256 per derived key, memoized via KeyCache."
```

---

### Task 3: Delete the storage feature; storage works without alloc

**Files:**
- Modify: `crates/sdk/Cargo.toml` (feature at line 63, docs.rs metadata 13-16, `[[test]]` blocks at 104-113)
- Modify: `crates/macros/Cargo.toml:28` + `crates/macros/src/lib.rs:55,57,157,174,199` (cfg gates)
- Modify: `crates/sdk/src/lib.rs:174-183`
- Modify: `crates/sdk/src/storage/bytestring.rs`, `rcq.rs` (per-method alloc gates)
- Audit: every `alloc::`/`Box`/`Vec` use under `crates/sdk/src/storage/` and the read paths they imply
- Modify: `scripts/dev.sh:266,407`

- [ ] **Step 1: Ungate the module and macros**

- sdk `Cargo.toml`: delete `storage = ["alloc", "groundhog-macros/storage"]`; add `"groundhog-macros/storage"`'s content by deleting the macros feature instead (next bullet); remove `"storage"` from `metadata.docs.rs.features`; delete the three `[[test]]` blocks for `storage`, `rcq_storage`, `compile_fail_storage` entirely (tests auto-discover; they needed the blocks only for `required-features`).
- macros `Cargo.toml`: delete `storage = []`; in `crates/macros/src/lib.rs` remove the five `#[cfg(feature = "storage")]` gates (the `deletable`/`storage` mods and the `Storage`/`Deletable` derives + `storage` attr become unconditional).
- sdk `src/lib.rs:174-183`: remove the four `#[cfg(feature = "storage")]` gates (storage mod + re-exports unconditional).

- [ ] **Step 2: Make storage's read paths work without alloc**

`cargo check -p sdk` (featureless) now compiles storage and will fail wherever storage code needs alloc. Resolve each:

- `keypath.rs` is already clean (Task 2).
- **Owned-return APIs stay alloc-only:** `Bytestring::get`/`try_get` (`Box<[u8]>`) and `RCQ` `consume_raw`/`try_consume_raw`/`peek_raw`/`try_peek_raw` get `#[cfg(feature = "alloc")]` on the methods (and their `use alloc::boxed::Box;` imports move under the same gates).
- **Typed reads must work featureless:** `Value::try_get`/`get` (and anything else decoding a dynamic-length bytestring, e.g. RCQ's typed consume/peek if it routes through an owned buffer) must read into a fixed stack buffer instead of `Box`. Pattern: read `crates/sdk/src/sys_exn/bytestring.rs` and `crates/sdk/src/internal/serialize.rs` first — the SDK already has featureless fixed-buffer machinery (`MUBuf<ENCODE_MAX>`, and the no-alloc calldata path reads syscall output into stack buffers). Mirror that: get the length (`bytestring_get_len`/`GetLen` syscall), bound-check against `ENCODE_MAX` (values can't exceed it — `Value::set` encodes through the same fixed buffer; verify that and note it in a comment), read into a stack `MUBuf`, decode from its slice. Where a featureless raw "read into caller buffer" wrapper is missing at the sys layer, add it next to the existing Box-returning one rather than open-coding syscalls in storage types.
- If both modes can share the fixed-buffer path cleanly, prefer ONE path (no cfg); keep the `Box` APIs as the alloc-only convenience layer.

- [ ] **Step 3: Gate check — featureless sdk compiles and storage tests pass**

Run: `cargo check -p sdk`
Expected: clean (this is the no-alloc storage proof).
Run: `cargo test -p sdk -Fstd --tests && cargo test -p groundhog-macros`
Expected: PASS (note `-Fstorage` is gone from the command — it no longer exists; `-Fstd` still needed for the `int128` test's `required-features`).

- [ ] **Step 4: Update dev.sh and docs invocations**

- `scripts/dev.sh:266`: `cargo test -p sdk -Fstorage,std --tests` → `cargo test -p sdk -Fstd --tests`
- `scripts/dev.sh:407`: `cargo doc --features=dev,storage` → `cargo doc --features=dev`
- `grep -n "storage" scripts/dev.sh .github/workflows/*.yml` — fix any other feature-flag mentions (CI calls dev.sh, so workflow files likely need nothing).

- [ ] **Step 5: Workspace Cargo.toml sweep**

For every manifest found by `grep -rln 'features = \[[^]]*"storage"' crates example-contracts integration_test standard-interfaces --include="Cargo.toml" | grep -v target` (~45 files): remove `"storage"` from the sdk dependency's feature list (keep `alloc`, `dev`, etc.; collapse to `sdk = { workspace = true }` if the list becomes empty). Same in the submodule. Re-grep to confirm zero hits.

- [ ] **Step 6: Full verify + commit**

Run: `cargo test -p sdk -Fstd --tests && cargo test -p groundhog-macros && ./scripts/dev.sh test-examples && cargo check --workspace --manifest-path integration_test/Cargo.toml 2>&1 | tail -5`
Expected: PASS (integration: only the pre-existing `hash_bug` failure).

```bash
git add crates scripts example-contracts standard-interfaces
git commit -m "Delete the storage Cargo feature; storage compiles with or without alloc

Typed reads route through the fixed encode-buffer machinery; the
Box-returning convenience APIs (Bytestring::get, RCQ raw reads) are
per-method alloc-gated. The alloc feature itself is unchanged."
# submodule:
# (cd integration_test/contracts && git add -A && git commit -m "Drop storage feature from sdk deps")
# then: git add integration_test/contracts (gitlink) and amend or include in the commit above
```

---

### Task 4: Contract sweep — drop gratuitous self receivers

**Files:** the bare-impl contracts migrated earlier (NOT the sdk trybuild fixtures — those exist to test the self-method machinery and keep `#[storage]`):
- example-contracts: `invoke_flags_test`
- integration_test (submodule): `authclaim_test`, `calldata_wrapper`, `cxs_roundtrip`*, `delegate_call_test`, `deploy_and_transfer`, `dev_wallet`, `initial_token_claim`, `manual_revert_test`*, `nnint_output_fault_test`*, `no_syscall_test`, `output_alias_test`*, `read_only_test`, `return_fault_test`, `sha256_wrapper`, `simple_wallet_authclaim`, `syscall_misc_tests`, `syscall_parse_hardening_test`*, `tuple_return`, `txs_roundtrip`*, `view_gateway_test` (* = disabled; migrate textually)

Trait impls keep `#[storage]` unit structs and trait-mandated receivers: `erc20_no_storage` (renamed in Task 5), `itc_erc20`, `erc1271_wallet`.

- [ ] **Step 1: Per contract, apply the rule**

If the impl is a bare (non-trait) impl and its methods' `self` receivers are never used for storage-struct field access (true for all listed — they're unit structs): delete the `&self`/`&mut self` parameter from each method, delete the `#[storage]`/`#[sdk::storage]` attribute from the unit struct, and remove any `features = ["storage"]` remnant the Task-3 sweep left (it should have caught Cargo.tomls already). Method bodies are unchanged (they never touched `self`). Contracts that genuinely use storage structs (e.g. `htlc`, `payment_channel`, `simple_wallet`, `fibonacci*`, `manual_revert_caller`) are NOT in the list — leave them alone.

- [ ] **Step 2: Keep one featureless self-method proof**

`itc_erc20` (trait impl, unit struct, raw-syscall state) must end this task with `sdk = { workspace = true }` — zero features — proving `#[storage]` + self receivers work without `alloc`/`storage`. If its manifest still has `alloc` for other reasons, check whether它 actually needs it; drop if not.

- [ ] **Step 3: Verify**

Run: `./scripts/dev.sh test-examples && cargo check --workspace --manifest-path integration_test/Cargo.toml 2>&1 | tail -5`
Expected: PASS / only pre-existing hash_bug failure. Entry-point IDs may have shifted where receivers left signatures — the contracts' own tests passing is the acceptance.

- [ ] **Step 4: Commit (parent + submodule, then gitlink bump)**

```bash
(cd integration_test/contracts && git add -A && git commit -m "Drop gratuitous self receivers from test contracts; bare structs, no #[storage]")
git add example-contracts integration_test/contracts
git commit -m "Drop gratuitous self receivers from bare-impl example contracts"
```

---

### Task 5: Rename erc20_no_storage → erc20_raw_storage

**Files:** `example-contracts/erc20_no_storage/` (whole dir), `example-contracts/Cargo.toml` (members), every reference from `grep -rn "erc20_no_storage" . --include="*.toml" --include="*.rs" --include="*.sh" --include="*.md" | grep -v target`

- [ ] **Step 1: Move and rename**

```bash
git mv example-contracts/erc20_no_storage example-contracts/erc20_raw_storage
```

Then: package `name = "erc20_no_storage"` → `erc20_raw_storage` in its Cargo.toml; update `example-contracts/Cargo.toml` members; update every grep hit (test wiring, dev.sh package lists, bottle-test deps, README/docs mentions). Update the crate's module doc to say it manages balances via raw storage keys/syscalls instead of the typed storage layer.

- [ ] **Step 2: Verify + commit**

Run: `./scripts/dev.sh test-examples` (package list must show `erc20_raw_storage`), then `grep -rn "erc20_no_storage" . | grep -v target | grep -v docs/superpowers` → empty.

```bash
git add -A example-contracts scripts README.md
git commit -m "Rename erc20_no_storage to erc20_raw_storage"
```

---

### Task 6: Corpus size gate + full verification + PR update

- [ ] **Step 1: Rebuild corpus and diff against Task 0**

```bash
./scripts/dev.sh build-examples && ./scripts/dev.sh build-itc
ls -l target/optimized/*.wasm | tr -s ' ' | cut -d' ' -f5,9 | sort -k2 > /tmp/wasm_sizes_after.txt
# account for the rename when diffing:
diff <(sed 's/erc20_no_storage/erc20_raw_storage/' /tmp/wasm_sizes_baseline.txt | sort -k2) /tmp/wasm_sizes_after.txt
```

Acceptance (spec §6): no contract grows > ~100 B without an identified cause; expect storage-free contracts to SHRINK (the LibraryExn::Storage codec left the abort path; framed keypaths add a few bytes to storage users). Tabulate the per-contract deltas in the report. If a storage-free contract grew materially, STOP and report — do not rationalize.

- [ ] **Step 2: Size guard + suites**

Run: `./scripts/check-wasm-size.sh` → pass.
Run: `cargo test -p sdk -Fstd --tests && cargo test -p groundhog-macros && cargo test && ./scripts/dev.sh test-examples && cargo check -p sdk && cargo fmt --all --check && cargo clippy -F dev --lib 2>&1 | grep -cE "^error"`
Expected: all pass; clippy error count 0.
Greps: `grep -rn 'feature = "storage"' crates | grep -v target` → empty; `grep -rln 'features = \[[^]]*"storage"' . --include="Cargo.toml" | grep -v target` → empty.

- [ ] **Step 3: Push (submodule first), update PR #240 description, drive CI + greptile**

```bash
(cd integration_test/contracts && git push)
git push
gh pr edit 240 --body "<append the storage-feature/Keypath/StorageError/rename summary to the existing body>"
gh pr checks 240 --watch
```

Then summon/await greptile on the new head and address findings.
