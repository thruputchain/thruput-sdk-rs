# Deletable Trait & Vector Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `Deletable` trait for type-safe recursive deletion, change Vector's length to `Value<u32>`, and add `pop()`.

**Architecture:** New `Deletable` trait with `delete()`, `delete_exclusive()`, and `delete_internal(flags)`. Leaf storage types implement by deleting their storage key. Compound types (Vector, Node, derive-macro structs) implement by recursively deleting children. Vector switches from NNInt-based length to Value<u32>, with u32 indices throughout.

**Tech Stack:** Rust, groundhog SDK storage layer, groundhog-macros proc-macro crate

**Spec:** `docs/superpowers/specs/2026-03-31-deletable-trait-vector-improvements.md`

**Build/test commands:**
- SDK check: `cargo check --features std`
- Storage tests: `cargo test --features std,storage --test storage`
- Compile-fail tests: `cargo test --features std,storage --test compile_fail_storage`
- Vector integration tests: `cd tests/vector-tests && cargo test`
- Example contracts: `cd example-contracts && cargo check -p erc20 -p complex-storage -p storage_errors`
- Format: `cargo fmt`

---

### Task 1: Add `delete_with_flags` to syscall layers

**Files:**
- Modify: `src/sys_result/delete.rs`
- Modify: `src/sys_exn/delete.rs`

- [ ] **Step 1: Add flags-based delete to sys_result**

```rust
// src/sys_result/delete.rs — add after existing functions:

pub fn delete_with_flags(
    key: &StorageKey,
    flags: StorageDeltaFlags,
) -> Result<(), SysErr> {
    sys::raw_DeleteObject(key, flags.0)
}
```

- [ ] **Step 2: Add flags-based delete to sys_exn**

```rust
// src/sys_exn/delete.rs — add after existing functions, before the `delete(key, bool)` fn:

use groundhog_xdr::storage_item::StorageDeltaFlags;

pub fn delete_with_flags(key: &StorageKey, flags: StorageDeltaFlags) {
    result::delete_with_flags(key, flags).unwrap_or_else(|errno| {
        crate::exn::syscall_throw(crate::sys::SyscallNo::kDeleteObject, errno)
    });
}
```

- [ ] **Step 3: Verify build**

Run: `cargo check --features std`
Expected: compiles clean

- [ ] **Step 4: Commit**

```
feat: add delete_with_flags to syscall layers
```

---

### Task 2: Create Deletable trait

**Files:**
- Create: `src/storage/deletable.rs`
- Modify: `src/storage/mod.rs`

- [ ] **Step 1: Create the trait file**

```rust
// src/storage/deletable.rs
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// Trait for storage types whose data can be fully deleted.
///
/// Leaf types (NNInt, Value, Bytestring) delete their single storage key.
/// Compound types implement this recursively when their elements are
/// Deletable. Types like Map that cannot enumerate entries do not
/// implement this trait.
pub trait Deletable {
    /// Delete storage for this node.
    ///
    /// Implementors OR the incoming `flags` with their keypath's own
    /// exclusive flag before issuing the syscall. This makes exclusivity
    /// additive: calling `delete()` on a slot accessed via
    /// `at_mut_exclusive()` still uses exclusive locking.
    fn delete_internal(&mut self, flags: StorageDeltaFlags);

    /// Delete using the keypath's own locking semantics.
    fn delete(&mut self) {
        self.delete_internal(StorageDeltaFlags::None);
    }

    /// Delete with an exclusive lock, regardless of keypath flag.
    fn delete_exclusive(&mut self) {
        self.delete_internal(StorageDeltaFlags::ExclusiveLock);
    }
}
```

- [ ] **Step 2: Wire into mod.rs**

In `src/storage/mod.rs`, add after the `pub mod node` / `pub use node::*` block:

```rust
pub mod deletable;
pub use deletable::*;
```

- [ ] **Step 3: Verify build**

Run: `cargo check --features std`
Expected: compiles clean (trait exists, no impls yet)

- [ ] **Step 4: Commit**

```
feat: add Deletable trait
```

---

### Task 3: Implement Deletable for NNInt, remove inherent delete

**Files:**
- Modify: `src/storage/nnint.rs`

The existing `NNInt::flags()` method already computes the keypath's `StorageDeltaFlags`. The `delete_internal` impl OR's the incoming flags with `self.flags()` using the raw `.0` fields.

- [ ] **Step 1: Add Deletable impl, remove inherent delete**

Remove the existing inherent `delete` method (lines 82-90) and add the trait impl:

```rust
impl super::deletable::Deletable for NNInt {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let effective = StorageDeltaFlags(flags.0 | self.flags().0);
        crate::sys_exn::delete::delete_with_flags(
            &self.keypath.generate_key(),
            effective,
        );
    }
}
```

- [ ] **Step 2: Verify build and tests**

Run: `cargo check --features std`
Run: `cargo test --features std,storage --test storage`
Expected: compiles, all tests pass. The `test_value_basic` test calls `.delete()` on a `Value`, not NNInt, so nothing breaks yet.

- [ ] **Step 3: Commit**

```
feat: implement Deletable for NNInt, remove inherent delete
```

---

### Task 4: Implement Deletable for Value\<T\>, remove inherent delete

**Files:**
- Modify: `src/storage/value.rs`

- [ ] **Step 1: Add Deletable impl, remove inherent delete**

Remove the existing inherent `delete` method block (lines 104-113: `impl<T> Value<T> { pub fn delete ... }`) and add:

```rust
impl<T> super::deletable::Deletable for Value<T> {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let keypath_flags = if self.key.keypath().is_exclusive() {
            StorageDeltaFlags::ExclusiveLock
        } else {
            StorageDeltaFlags::None
        };
        let effective = StorageDeltaFlags(flags.0 | keypath_flags.0);
        crate::sys_exn::delete::delete_with_flags(&self.key, effective);
    }
}
```

Add `use groundhog_xdr::storage_item::StorageDeltaFlags;` to the imports at the top of `value.rs`.

- [ ] **Step 2: Verify build and tests**

Run: `cargo test --features std,storage --test storage`
Expected: all tests pass. The `test_value_basic` test calls `val.delete()` — this now dispatches through the trait's default `delete()` method.

- [ ] **Step 3: Commit**

```
feat: implement Deletable for Value<T>, remove inherent delete
```

---

### Task 5: Implement Deletable for Bytestring, remove inherent delete

**Files:**
- Modify: `src/storage/bytestring.rs`

- [ ] **Step 1: Add Deletable impl, remove inherent delete**

Remove the existing inherent `delete` method (lines 45-51) and add:

```rust
impl super::deletable::Deletable for Bytestring {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let keypath_flags = if self.key.keypath().is_exclusive() {
            StorageDeltaFlags::ExclusiveLock
        } else {
            StorageDeltaFlags::None
        };
        let effective = StorageDeltaFlags(flags.0 | keypath_flags.0);
        crate::sys_exn::delete::delete_with_flags(self.key.get(), effective);
    }
}
```

Add `use groundhog_xdr::storage_item::StorageDeltaFlags;` to the imports.

- [ ] **Step 2: Verify build and tests**

Run: `cargo check --features std`
Expected: compiles clean

- [ ] **Step 3: Commit**

```
feat: implement Deletable for Bytestring, remove inherent delete
```

---

### Task 6: Implement Deletable for Node\<T\>

**Files:**
- Modify: `src/storage/node.rs`

- [ ] **Step 1: Add conditional Deletable impl**

```rust
impl<T: super::deletable::Deletable> super::deletable::Deletable for Node<T> {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        self.fields.delete_internal(flags);
    }
}
```

Add `use groundhog_xdr::storage_item::StorageDeltaFlags;` to the imports.

- [ ] **Step 2: Verify build**

Run: `cargo check --features std`
Expected: compiles clean

- [ ] **Step 3: Commit**

```
feat: implement Deletable for Node<T: Deletable>
```

---

### Task 7: Generate Deletable impl in derive macro

**Files:**
- Modify: `macros/src/storage.rs`

The derive macro currently generates `ReadXDRPath` and `From<Keypath>` impls. Add a conditional `Deletable` impl with where clauses on each field type.

- [ ] **Step 1: Add Deletable codegen to `generate_from_struct`**

In `macros/src/storage.rs`, add a new function and call it from `generate_from_struct`:

```rust
fn impl_deletable(
    ident: &syn::Ident,
    generics: &syn::Generics,
    fields: &syn::Fields,
) -> Result<TokenStream> {
    let ty = generic_type_name(ident, generics);
    let generic_params = &generics.params;

    let named_fields = match fields {
        syn::Fields::Named(fields_named) => &fields_named.named,
        syn::Fields::Unit => {
            // Unit struct: nothing to delete
            return Ok(quote! {
                const _: () = {
                    use sdk::storage::Deletable;
                    use sdk::StorageDeltaFlags;

                    impl<#generic_params> Deletable for #ty {
                        fn delete_internal(
                            &mut self,
                            _flags: StorageDeltaFlags,
                        ) {}
                    }
                };
            });
        }
        _ => return err(fields, "Can only handle named or unit structs"),
    };

    let mut where_clauses = TokenStream::new();
    let mut delete_calls = TokenStream::new();

    for field in named_fields.iter() {
        let field_name = &field.ident;
        let field_type = &field.ty;
        where_clauses.extend(quote! { #field_type: Deletable, });
        delete_calls.extend(quote! {
            self.#field_name.delete_internal(flags);
        });
    }

    Ok(quote! {
        const _: () = {
            use sdk::storage::Deletable;
            use sdk::StorageDeltaFlags;

            impl<#generic_params> Deletable for #ty
            where
                #where_clauses
            {
                fn delete_internal(
                    &mut self,
                    flags: StorageDeltaFlags,
                ) {
                    #delete_calls
                }
            }
        };
    })
}
```

Then in `generate_from_struct`, add a call:

```rust
fn generate_from_struct(
    input: &syn::DeriveInput,
    item: &syn::DataStruct,
) -> Result<TokenStream> {
    let ident = &input.ident;
    let generics = &input.generics;
    let fields = &item.fields;

    let mut output = TokenStream::new();
    output.extend(impl_read_xdr_path(ident, generics, fields)?);
    output.extend(impl_from_keypath(ident, generics, fields)?);
    output.extend(impl_deletable(ident, generics, fields)?);

    Ok(output)
}
```

- [ ] **Step 2: Verify build**

Run: `cargo check --features std`
Expected: compiles clean. Existing `#[derive(Storage)]` structs like `Data` in `tests/storage.rs` get a Deletable impl. Structs with Map fields (like `InnerStorage` in complex-storage) get the impl but it's unsatisfied because `Map` isn't Deletable — no error unless used.

- [ ] **Step 3: Commit**

```
feat: derive macro generates conditional Deletable impl
```

---

### Task 8: Change Vector length to Value\<u32\> and index type to u32

**Files:**
- Modify: `src/storage/vector.rs`
- Modify: `tests/storage.rs`
- Modify: `tests/vector-tests/src/lib.rs`
- Modify: `example-contracts/complex-storage/src/lib.rs`
- Modify: `example-contracts/storage_errors/src/lib.rs`

- [ ] **Step 1: Rewrite vector.rs**

Replace the full contents of `src/storage/vector.rs`:

```rust
use super::keypath::Keypath;
use super::map::Map;
use super::node::StorageType;
use super::slot::{SlotMut, SlotRef};
use super::value::Value;
use core::fmt::Debug;

pub struct Vector<S> {
    keypath: Keypath,
    len: Value<u32>,
    values: Map<u32, S>,
}

impl<S> Debug for Vector<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Vector")
            .field("keypath", &self.keypath)
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

impl<S> From<Keypath> for Vector<S> {
    fn from(keypath: Keypath) -> Self {
        let mut len_kp = keypath.clone();
        len_kp.seq_begin();
        len_kp.set_exclusive();

        let mut values_kp = keypath.clone();
        values_kp.seq_begin();
        values_kp.seq_next();

        Self {
            keypath,
            len: Value::from(len_kp),
            values: Map::from(values_kp),
        }
    }
}

impl_load_via_keypath! { Vector<S> }
impl_read_via_keypath! { Vector<S> }

impl<S> super::sealed::Sealed for Vector<S> {}
impl<S> super::node::StorageType for Vector<S> {}

impl<S> Vector<S> {
    pub fn keypath(&self) -> &Keypath {
        &self.keypath
    }
}

impl<S: StorageType> Vector<S> {
    /// Current length (number of elements). Returns 0 if never written.
    pub fn len(&self) -> u32 {
        self.len.try_get().unwrap_or(0)
    }

    /// Returns true if the vector contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Borrow element at `index` (bounds-checked).
    pub fn at(&self, index: u32) -> SlotRef<'_, Self, S> {
        self.bounds_check(index);
        let keypath = self.make_elem_keypath(index);
        let slot = S::from(keypath);
        SlotRef::new(self, slot)
    }

    /// Mutably borrow element at `index` (bounds-checked).
    pub fn at_mut(&mut self, index: u32) -> SlotMut<'_, Self, S> {
        self.bounds_check(index);
        let keypath = self.make_elem_keypath(index);
        let slot = S::from(keypath);
        SlotMut::new(self, slot)
    }

    /// Allocate a new slot at the end and return a mutable reference to it.
    pub fn push(&mut self) -> SlotMut<'_, Self, S> {
        let idx = self.len();
        let new_len = idx.checked_add(1).expect("vector length overflow");
        self.len.set(&new_len);
        let keypath = self.make_elem_keypath(idx);
        let slot = S::from(keypath);
        SlotMut::new(self, slot)
    }

    fn bounds_check(&self, index: u32) {
        let len = self.len();
        if index >= len {
            super::error::oob_throw(len as u64, index as u64);
        }
    }

    fn make_elem_keypath(&self, index: u32) -> Keypath {
        let mut kp = self.values.keypath().clone();
        kp.push_item(&index);
        kp
    }
}
```

- [ ] **Step 2: Update tests/storage.rs**

Change `test_storable`:
```rust
// Change the read loop from u64 to u32 indices:
    for i in 0..3u32 {
        let n = data.nums.at(i).get();
        assert_eq!(n, i as u64);
    }
```

Change `test_vector`:
```rust
    assert_eq!(3, vec.len());
    for i in 0..3u32 {
        assert_eq!(i, vec.at(i).get());
    }
```

- [ ] **Step 3: Update tests/vector-tests/src/lib.rs**

Change all `u64` index parameters and return types to `u32`:

```rust
    pub fn get_value(&self, index: u32) -> u64 { ... }
    pub fn get_value_or_zero(&self, index: u32) -> u64 { ... }
    pub fn values_len(&self) -> u32 { ... }
    pub fn inc_nnint(&mut self, index: u32, delta: i128) { ... }
    pub fn get_nnint(&self, index: u32) -> i128 { ... }
    pub fn nnints_len(&self) -> u32 { ... }
    pub fn nested_get(&self, key: u32, index: u32) -> u64 { ... }
    pub fn nested_len(&self, key: u32) -> u32 { ... }
```

- [ ] **Step 4: Update example-contracts/complex-storage/src/lib.rs**

Change the two `self.inners.len() - 1` inline usages to a local variable (the subtraction result is now u32, which is fine, but the expression `self.inners.at_mut(self.inners.len() - 1)` borrows self twice):

```rust
    pub fn push_inner(&mut self) {
        self.inners.push();
        let last = self.inners.len() - 1;
        let mut inner = self.inners.at_mut(last);
        ...
    }

    pub fn get_inner_balance(&self, addr: ContractAddress) -> u64 {
        assert!(!self.inners.is_empty(), "No inner storage");
        let last = self.inners.len() - 1;
        let inner = self.inners.at(last);
        ...
    }
```

- [ ] **Step 5: Update example-contracts/storage_errors/src/lib.rs**

Change vector method signatures from `u64` to `u32`:

```rust
    pub fn vector_get(&self, index: u32) -> u64 { ... }
    pub fn vector_set(&mut self, index: u32, value: u64) { ... }
    pub fn vector_len(&self) -> u32 { ... }
```

- [ ] **Step 6: Verify all tests**

Run: `cargo test --features std,storage --test storage`
Run: `cargo test --features std,storage --test compile_fail_storage`
Run: `cd tests/vector-tests && cargo test`
Run: `cd example-contracts && cargo check -p erc20 -p complex-storage -p storage_errors`
Expected: all pass

- [ ] **Step 7: Format and commit**

Run: `cargo fmt`

```
refactor: Vector uses Value<u32> for length, u32 indices
```

---

### Task 9: Implement Deletable for Vector, add pop()

**Files:**
- Modify: `src/storage/vector.rs`

- [ ] **Step 1: Add Deletable impl and pop()**

Add to the bottom of `src/storage/vector.rs`, after the existing `impl<S: StorageType> Vector<S>` block:

```rust
use super::deletable::Deletable;
use groundhog_xdr::storage_item::StorageDeltaFlags;

impl<S: StorageType + Deletable> Vector<S> {
    /// Remove the last element, deleting its storage.
    /// Aborts if the vector is empty.
    pub fn pop(&mut self) {
        let len = self.len();
        if len == 0 {
            super::error::oob_throw(0, 0);
        }
        let new_len = len - 1;
        let mut elem = S::from(self.make_elem_keypath(new_len));
        elem.delete_internal(StorageDeltaFlags::ExclusiveLock);
        self.len.set(&new_len);
    }
}

impl<S: StorageType + Deletable> Deletable for Vector<S> {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let len = self.len();
        for i in 0..len {
            let mut elem = S::from(self.make_elem_keypath(i));
            elem.delete_internal(flags);
        }
        self.len.delete_internal(flags);
    }
}
```

- [ ] **Step 2: Verify build**

Run: `cargo check --features std`
Expected: compiles clean

- [ ] **Step 3: Commit**

```
feat: implement Deletable for Vector, add pop()
```

---

### Task 10: Add tests for Deletable and pop()

**Files:**
- Modify: `tests/storage.rs`
- Modify: `tests/vector-tests/src/lib.rs`
- Modify: `tests/vector-tests/tests/test.rs`

- [ ] **Step 1: Add pop and delete tests to tests/storage.rs**

Add at the end of `tests/storage.rs` (before the last `test_data_fully_consumed` test):

```rust
contract_test!(test_vector_pop {
    let mut vec: Vector<Value<u32>> = Keypath::root().into();

    vec.push().set(&10);
    vec.push().set(&20);
    vec.push().set(&30);
    assert_eq!(3, vec.len());

    vec.pop();
    assert_eq!(2, vec.len());

    // Popped element's storage is deleted
    assert_eq!(vec.at(0).get(), 10);
    assert_eq!(vec.at(1).get(), 20);

    vec.pop();
    vec.pop();
    assert_eq!(0, vec.len());
});

contract_test!(test_vector_delete {
    let mut vec: Vector<Value<u32>> = Keypath::root().into();

    vec.push().set(&1);
    vec.push().set(&2);
    vec.push().set(&3);

    vec.delete();
    assert_eq!(0, vec.len());
});

contract_test!(test_nnint_delete {
    let keypath = Keypath::new();
    let mut n: NNInt = keypath.into();
    n.set(42);
    assert_eq!(n.get(), 42);
    n.delete();
    assert_eq!(n.try_get(), None);
});

contract_test!(test_vector_pop_nnint {
    let keypath = Keypath::new();
    let mut vec: Vector<NNInt> = keypath.into();

    vec.push().set(10);
    vec.push().set(20);
    assert_eq!(2, vec.len());

    vec.pop();
    assert_eq!(1, vec.len());
    assert_eq!(vec.at(0).get(), 10);
});
```

- [ ] **Step 2: Add pop methods to vector-tests contract**

In `tests/vector-tests/src/lib.rs`, add to the contract impl:

```rust
    pub fn pop_value(&mut self) {
        self.values.pop();
    }

    pub fn pop_nnint(&mut self) {
        self.nnints.pop();
    }
```

- [ ] **Step 3: Add pop integration test**

In `tests/vector-tests/tests/test.rs`, add a new test:

```rust
#[test]
fn test_vector_pop() {
    let (env, contract_hash) = setup();
    let addr = Cell::new(ContractAddress::default());

    let deploy_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });
    deploy_result.assert_success();

    env.advance_block();

    // Push two values, pop one, verify
    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = VectorTestClient::new(&contract_addr);

                client.push_value(100);
                client.push_value(200);
                assert_eq!(client.values_len(), 2);

                client.pop_value();
                assert_eq!(client.values_len(), 1);
                assert_eq!(client.get_value(0), 100);
            }))
    });
    result.assert_success();
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo test --features std,storage --test storage`
Run: `cd tests/vector-tests && cargo test`
Expected: all pass

- [ ] **Step 5: Format and commit**

Run: `cargo fmt`

```
test: add tests for Deletable trait and Vector::pop()
```

---

### Task 11: Update integration test contracts that call .delete()

**Files:**
- Modify: `integration_test/contracts/htlc/src/implementation.rs`
- Modify: `integration_test/contracts/payment_channel/src/implementation.rs`

These files call `.delete()` on storage slots. Since the inherent `delete()` methods were removed, these now resolve to the trait's default `delete()` method. They should continue to work without changes as long as `Deletable` is in scope (it's re-exported via `sdk::storage::*`).

- [ ] **Step 1: Verify integration contracts compile**

Check that the `Deletable` trait is in scope for these files. Since they use `sdk::*` or `sdk::storage::*`, and `Deletable` is `pub use`'d from `storage/mod.rs`, the trait's `delete()` default method should be found automatically.

Run: `cd integration_test && cargo check` (if possible) or `cargo check --features std` from the SDK root.

If compilation fails because the trait isn't in scope, add `use sdk::storage::Deletable;` to the affected files.

- [ ] **Step 2: Commit (if changes needed)**

```
fix: ensure Deletable trait in scope for integration contracts
```

---

### Task 12: Final verification

- [ ] **Step 1: Run full test suite**

```bash
cargo fmt
cargo test --features std,storage --test storage
cargo test --features std,storage --test compile_fail_storage
cargo test --features std --test nnints
cd tests/vector-tests && cargo test && cd ../..
cd example-contracts && cargo check -p erc20 -p complex-storage -p storage_errors && cd ..
```

All must pass.

- [ ] **Step 2: Verify no leftover inherent delete methods**

Search for inherent `delete` methods that should have been removed:

```bash
# Should find ZERO matches in nnint.rs, value.rs, bytestring.rs:
grep -n "pub fn delete" src/storage/nnint.rs src/storage/value.rs src/storage/bytestring.rs
```

The only `delete`-related code in those files should be the `Deletable` trait impls.

- [ ] **Step 3: Commit any final fixups, if needed**
