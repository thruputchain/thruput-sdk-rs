# Unified Storage Slot API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Map's get/set/entry API with at()/at_mut() borrow guards, unify storage types under StorageNode, and centralize exclusive lock decisions at slot acquisition time.

**Architecture:** Collections become slot factories — `at()` / `at_mut()` return `SlotRef` / `SlotMut` guards that deref to the StorageNode. All storage types (NNInt, Bytestring, Value<T>) have immediate-syscall methods gated by `&self` (reads) / `&mut self` (writes). Value<T> replaces Handle<T>, adding `ValueMut` for in-place modification with flush-on-drop. Exclusive lock flag lives on Keypath and propagates through nesting.

**Tech Stack:** Rust (no_std + alloc), proc macros (groundhog_macros), XDR serialization (xdrs)

**Spec:** `docs/superpowers/specs/2026-03-29-unified-storage-slot-api.md`

---

### Task 1: Add exclusive flag to Keypath

**Files:**
- Modify: `src/storage/keypath.rs`

- [ ] **Step 1: Add `exclusive` field to Keypath**

```rust
// src/storage/keypath.rs — modify the struct (line 8)
#[derive(Debug, Clone)]
pub struct Keypath {
    path: Vec<KeypathComponent>,
    exclusive: bool,
}
```

- [ ] **Step 2: Update all Keypath constructors to initialize `exclusive: false`**

```rust
// Keypath::new() (line 19)
pub fn new() -> Self {
    Self { path: vec![], exclusive: false }
}

// Keypath::clone_flattened() (line 33)
pub fn clone_flattened(&self) -> Self {
    Keypath {
        path: vec![KeypathComponent::Bytes(self.flattened())],
        exclusive: self.exclusive,
    }
}
```

- [ ] **Step 3: Add exclusive getter/setter**

```rust
// Add to impl Keypath, after seq_end()
pub fn is_exclusive(&self) -> bool {
    self.exclusive
}

pub fn set_exclusive(&mut self) {
    self.exclusive = true;
}
```

- [ ] **Step 4: Run existing tests to verify nothing is broken**

Run: `cargo test --lib -- test_keypath`
Expected: PASS (clone preserves exclusive flag via `#[derive(Clone)]`)

- [ ] **Step 5: Add test for exclusive flag propagation**

```rust
// Add to mod test_keypath in src/storage/keypath.rs
#[test]
fn exclusive_flag_propagates_through_clone() {
    let mut kp = Keypath::new();
    kp.push_item("some bytes");
    kp.set_exclusive();

    let kp2 = kp.clone();
    assert!(kp2.is_exclusive());

    let kp3 = kp.clone_flattened();
    assert!(kp3.is_exclusive());
}

#[test]
fn exclusive_flag_defaults_to_false() {
    let kp = Keypath::new();
    assert!(!kp.is_exclusive());
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test --lib -- test_keypath`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/storage/keypath.rs
git commit -m "feat: add exclusive flag to Keypath for lock propagation"
```

---

### Task 2: Add StorageNode trait

**Files:**
- Create: `src/storage/node.rs`
- Modify: `src/storage/mod.rs`
- Modify: `src/storage/nnint.rs`
- Modify: `src/storage/bytestring.rs`
- Modify: `src/storage/map.rs`
- Modify: `src/storage/vector.rs`

- [ ] **Step 1: Create `src/storage/node.rs` with the trait**

```rust
// src/storage/node.rs
use super::keypath::Keypath;

/// Unifying trait for all storage-backed types.
///
/// Storage nodes are lightweight handles constructed from a keypath.
/// They may be leaf types (NNInt, Bytestring, Value<T>) whose methods
/// make immediate syscalls, or collection types (Map, Vector) that
/// produce child nodes via `at()` / `at_mut()`.
pub trait StorageNode: Sized {
    fn from_keypath(keypath: Keypath) -> Self;
    fn keypath(&self) -> &Keypath;
}
```

- [ ] **Step 2: Register the module in `src/storage/mod.rs`**

Add after the `pub mod store;` / `pub use store::*;` block:

```rust
pub mod node;
pub use node::*;
```

- [ ] **Step 3: Implement StorageNode for NNInt**

Add to `src/storage/nnint.rs` after the existing `impl NNInt` block:

```rust
impl super::node::StorageNode for NNInt {
    fn from_keypath(keypath: super::keypath::Keypath) -> Self {
        Self::from(keypath)
    }

    fn keypath(&self) -> &super::keypath::Keypath {
        &self.keypath
    }
}
```

- [ ] **Step 4: Implement StorageNode for Bytestring**

Add to `src/storage/bytestring.rs` after the existing `impl Bytestring` block:

```rust
impl super::node::StorageNode for Bytestring {
    fn from_keypath(keypath: super::keypath::Keypath) -> Self {
        Self::from(keypath)
    }

    fn keypath(&self) -> &super::keypath::Keypath {
        self.key.keypath()
    }
}
```

- [ ] **Step 5: Implement StorageNode for Map**

Add to `src/storage/map.rs` after the existing `impl<K, V> Map<K, V>` block:

```rust
impl<K, V> super::node::StorageNode for Map<K, V> {
    fn from_keypath(keypath: super::keypath::Keypath) -> Self {
        Self::from(keypath)
    }

    fn keypath(&self) -> &super::keypath::Keypath {
        &self.keypath
    }
}
```

- [ ] **Step 6: Implement StorageNode for Vector**

Add to `src/storage/vector.rs` after the existing `impl<T> Vector<T>` block:

```rust
impl<T> super::node::StorageNode for Vector<T> {
    fn from_keypath(keypath: super::keypath::Keypath) -> Self {
        Self::from(keypath)
    }

    fn keypath(&self) -> &super::keypath::Keypath {
        &self.keypath
    }
}
```

- [ ] **Step 7: Verify it compiles**

Run: `cargo check`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add src/storage/node.rs src/storage/mod.rs src/storage/nnint.rs src/storage/bytestring.rs src/storage/map.rs src/storage/vector.rs
git commit -m "feat: add StorageNode trait with impls for all storage types"
```

---

### Task 3: Add SlotRef and SlotMut guard types

**Files:**
- Create: `src/storage/slot.rs`
- Modify: `src/storage/mod.rs`

- [ ] **Step 1: Create `src/storage/slot.rs`**

```rust
// src/storage/slot.rs
use super::node::StorageNode;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

/// Read-only borrowed slot. Holds a shared borrow on the parent collection.
pub struct SlotRef<'a, Owner, S: StorageNode> {
    slot: S,
    _borrow: PhantomData<&'a Owner>,
}

impl<'a, Owner, S: StorageNode> SlotRef<'a, Owner, S> {
    pub fn new(_owner: &'a Owner, slot: S) -> Self {
        Self {
            slot,
            _borrow: PhantomData,
        }
    }
}

impl<Owner, S: StorageNode> Deref for SlotRef<'_, Owner, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.slot
    }
}

/// Mutable borrowed slot. Holds an exclusive borrow on the parent collection.
pub struct SlotMut<'a, Owner, S: StorageNode> {
    slot: S,
    _borrow: PhantomData<&'a mut Owner>,
}

impl<'a, Owner, S: StorageNode> SlotMut<'a, Owner, S> {
    pub fn new(_owner: &'a mut Owner, slot: S) -> Self {
        Self {
            slot,
            _borrow: PhantomData,
        }
    }
}

impl<Owner, S: StorageNode> Deref for SlotMut<'_, Owner, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.slot
    }
}

impl<Owner, S: StorageNode> DerefMut for SlotMut<'_, Owner, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.slot
    }
}
```

- [ ] **Step 2: Register in `src/storage/mod.rs`**

Add after the `node` module:

```rust
pub mod slot;
pub use slot::*;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/storage/slot.rs src/storage/mod.rs
git commit -m "feat: add SlotRef and SlotMut borrow guard types"
```

---

### Task 4: Add at()/at_mut()/at_mut_exclusive() to Map

**Files:**
- Modify: `src/storage/map.rs`
- Modify: `tests/storage.rs`

- [ ] **Step 1: Write failing test for `at()` read access**

Add to `tests/storage.rs`:

```rust
contract_test!(test_map_at {
    let data_keypath = Keypath::new();
    {
        let data: Data = DefaultFromKeypath::default_from_keypath(&data_keypath);
        data.store_keypath(&data_keypath);
    }

    let slot: u32 = 3;
    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();

        // Use at_mut to write
        data.map.at_mut(&slot).strict_inc(7);
    }

    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();

        // Use at to read
        let v = data.map.at(&slot).get();
        assert_eq!(v, 7);
    }
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_map_at`
Expected: FAIL — `at` and `at_mut` methods don't exist on Map

- [ ] **Step 3: Implement at(), at_mut(), at_mut_exclusive() on Map**

Add to `impl<K, V> Map<K, V>` in `src/storage/map.rs`:

```rust
use super::slot::{SlotRef, SlotMut};
use super::node::StorageNode;
```

Then add these methods inside the existing `impl<K, V> Map<K, V>` block:

```rust
    pub fn at(&self, entry_key: &K) -> SlotRef<'_, Self, V>
    where
        K: WriteXDR,
        V: StorageNode,
    {
        let keypath = self.make_entry_keypath(entry_key);
        let slot = V::from_keypath(keypath);
        SlotRef::new(self, slot)
    }

    pub fn at_mut(&mut self, entry_key: &K) -> SlotMut<'_, Self, V>
    where
        K: WriteXDR,
        V: StorageNode,
    {
        let keypath = self.make_entry_keypath(entry_key);
        let slot = V::from_keypath(keypath);
        SlotMut::new(self, slot)
    }

    pub fn at_mut_exclusive(&mut self, entry_key: &K) -> SlotMut<'_, Self, V>
    where
        K: WriteXDR,
        V: StorageNode,
    {
        let mut keypath = self.make_entry_keypath(entry_key);
        keypath.set_exclusive();
        let slot = V::from_keypath(keypath);
        SlotMut::new(self, slot)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_map_at`
Expected: PASS

- [ ] **Step 5: Write test for nested map chaining**

Add to `tests/storage.rs`:

```rust
contract_test!(test_map_at_nested {
    let data_keypath = Keypath::new();
    {
        let data: Data = DefaultFromKeypath::default_from_keypath(&data_keypath);
        data.store_keypath(&data_keypath);
    }

    // Use the ERC20-like pattern: Map<CA, Map<CA, NNInt>>
    // We'll simulate with Data's map field (Map<u32, NNInt>)
    let slot: u32 = 5;
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();

        // at_mut returns SlotMut, which DerefMut gives &mut NNInt
        data.map.at_mut(&slot).strict_inc(42);
    }

    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();

        // at returns SlotRef, which Deref gives &NNInt
        let val = data.map.at(&slot).try_get().unwrap_or(0);
        assert_eq!(val, 42);
    }
});
```

- [ ] **Step 6: Run test**

Run: `cargo test test_map_at_nested`
Expected: PASS

- [ ] **Step 7: Write test verifying borrow guard prevents double mut access**

This test verifies the design intent at the type level. It should NOT compile. Add a comment in `tests/storage.rs` documenting the expected compile error:

```rust
// The following should NOT compile — SlotMut borrows &mut map exclusively:
//
//   let mut a = data.map.at_mut(&1u32);
//   let mut b = data.map.at_mut(&2u32);  // ERROR: cannot borrow `data.map` as mutable more than once
//   a.strict_inc(1);
//   b.strict_inc(1);
//
// And SlotRef + SlotMut should conflict:
//
//   let a = data.map.at(&1u32);
//   let mut b = data.map.at_mut(&2u32);  // ERROR: cannot borrow `data.map` as mutable while borrowed as immutable
//   let _ = a.get();
```

- [ ] **Step 8: Run all storage tests to verify nothing regressed**

Run: `cargo test --test storage`
Expected: PASS (all existing tests still pass, new tests pass)

- [ ] **Step 9: Commit**

```bash
git add src/storage/map.rs tests/storage.rs
git commit -m "feat: add at()/at_mut()/at_mut_exclusive() to Map"
```

---

### Task 5: Create Value\<T\> and ValueMut\<T\>

**Files:**
- Create: `src/storage/value.rs`
- Modify: `src/storage/mod.rs`
- Modify: `src/storage/handle.rs`

- [ ] **Step 1: Create `src/storage/value.rs`**

```rust
// src/storage/value.rs
use super::keypath::*;
use super::node::StorageNode;
use core::fmt::Debug;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

/// A typed storage slot for serialized values.
///
/// `Value<T>` is a handle to a storage location that holds a serialized `T`.
/// All operations are immediate syscalls — no buffering.
/// For in-place modification with auto-flush, use `load_mut()`.
#[derive(Debug)]
pub struct Value<T> {
    key: KeyCache,
    _marker: PhantomData<fn() -> T>,
}

impl<T> From<Keypath> for Value<T> {
    fn from(keypath: Keypath) -> Self {
        Self {
            key: KeyCache::from(keypath),
            _marker: PhantomData,
        }
    }
}

impl_load_via_keypath! { Value<T> }
impl_default_via_keypath! { Value<T> }
impl_read_via_keypath! { Value<T> }
impl_write_xdr_empty! { Value<T> }

impl<T> StorageNode for Value<T> {
    fn from_keypath(keypath: Keypath) -> Self {
        Self::from(keypath)
    }

    fn keypath(&self) -> &Keypath {
        self.key.keypath()
    }
}

impl<T> Value<T> {
    pub fn new(keypath: &Keypath) -> Self {
        Self {
            key: KeyCache::new(keypath),
            _marker: PhantomData,
        }
    }
}

impl<T> Value<T>
where
    T: super::load::LoadKeypath,
{
    /// Deserialize and return the stored value.
    /// Aborts if the key does not exist.
    pub fn get(&self) -> T {
        super::load::LoadKeypath::load_keypath(self.key.keypath())
            .unwrap_or_else(|e| crate::exn::library_throw(e))
    }

    /// Deserialize and return the stored value, or None if the key doesn't exist.
    pub fn try_get(&self) -> Option<T> {
        super::load::LoadKeypath::try_load_keypath(self.key.keypath())
            .unwrap_or_else(|e| crate::exn::library_throw(e))
    }
}

impl<T> Value<T>
where
    T: super::store::Store,
{
    /// Serialize and store the value immediately.
    pub fn set(&mut self, value: &T) {
        if self.key.keypath().is_exclusive() {
            value.store_exclusive(&self.key);
        } else {
            value.store(&self.key);
        }
    }
}

impl<T> Value<T> {
    /// Delete the value at this storage location.
    pub fn delete(&mut self) {
        if self.key.keypath().is_exclusive() {
            crate::delete_exclusive(&self.key);
        } else {
            crate::delete_shared(&self.key);
        }
    }
}

impl<T> Value<T>
where
    T: super::load::LoadKeypath + DefaultFromKeypath + super::store::Store,
{
    /// Load the value for in-place modification.
    /// Returns a `ValueMut` that derefs to `&mut T` and flushes on drop.
    pub fn load_mut(&mut self) -> ValueMut<'_, T> {
        let data = T::try_load_keypath(self.key.keypath())
            .unwrap_or_else(|e| crate::exn::library_throw(e))
            .unwrap_or_else(|| T::default_from_keypath(self.key.keypath()));
        ValueMut {
            value: self,
            data,
        }
    }
}

/// Mutable view into a `Value<T>`. Flushes T back to storage on drop.
pub struct ValueMut<'a, T: super::store::Store> {
    value: &'a mut Value<T>,
    data: T,
}

impl<T: super::store::Store> Deref for ValueMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.data
    }
}

impl<T: super::store::Store> DerefMut for ValueMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

impl<T: super::store::Store> Drop for ValueMut<'_, T> {
    fn drop(&mut self) {
        self.value.set(&self.data);
    }
}
```

- [ ] **Step 2: Register in `src/storage/mod.rs`**

Add after the `handle` module:

```rust
pub mod value;
pub use value::*;
```

- [ ] **Step 3: Add Handle alias in `src/storage/handle.rs`**

Add at the bottom of `src/storage/handle.rs` for backward compatibility. (The full Handle removal will happen in a later task.)

```rust
// Backward compatibility: Value<T> is the new name for Handle<T>'s role
// in collections. Handle<T> is retained for now but will be deprecated.
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check`
Expected: PASS

- [ ] **Step 5: Write test for Value\<T\> basic operations**

Add to `tests/storage.rs`:

```rust
contract_test!(test_value_basic {
    let keypath = Keypath::new();
    let mut val: Value<i32> = Value::new(&keypath);

    // Initially empty
    assert_eq!(val.try_get(), None);

    // Set a value
    val.set(&42);
    assert_eq!(val.get(), 42);

    // Overwrite
    val.set(&99);
    assert_eq!(val.get(), 99);

    // Delete
    val.delete();
    assert_eq!(val.try_get(), None);
});
```

- [ ] **Step 6: Run test**

Run: `cargo test test_value_basic`
Expected: PASS

- [ ] **Step 7: Write test for ValueMut flush-on-drop**

This test requires a struct type. Use the existing `Thing` type from the test file, or define one:

```rust
contract_test!(test_value_load_mut {
    let keypath = Keypath::new();
    let mut val: Value<i32> = Value::new(&keypath);

    // Set initial value
    val.set(&10);

    // Modify via load_mut — changes flush on drop
    {
        let mut data = val.load_mut();
        *data = 42;
    }

    assert_eq!(val.get(), 42);
});
```

- [ ] **Step 8: Run test**

Run: `cargo test test_value_load_mut`
Expected: PASS

- [ ] **Step 9: Write test for Value\<T\> through Map::at_mut()**

Add the `Value` import if needed and add to `tests/storage.rs`. First, update the `Data` struct to include a `Value<i32>` map, or use `map2` which is `Map<u32, i32>`.

Since `Map<u32, i32>` requires `i32: StorageNode` which we haven't implemented, use `Map<u32, Value<i32>>` in a standalone test:

```rust
contract_test!(test_map_at_value {
    use sdk::storage::Value;

    let keypath = Keypath::new();
    let mut map: Map<u32, Value<i32>> = Map::new(&keypath);

    // Write via at_mut
    map.at_mut(&1u32).set(&100);

    // Read via at
    let v = map.at(&1u32).try_get();
    assert_eq!(v, Some(100));

    // Modify via load_mut through at_mut
    {
        let mut slot = map.at_mut(&1u32);
        let mut data = slot.load_mut();
        *data = 200;
    }

    let v = map.at(&1u32).get();
    assert_eq!(v, 200);
});
```

- [ ] **Step 10: Run test**

Run: `cargo test test_map_at_value`
Expected: PASS

- [ ] **Step 11: Commit**

```bash
git add src/storage/value.rs src/storage/mod.rs tests/storage.rs
git commit -m "feat: add Value<T> and ValueMut<T> storage types"
```

---

### Task 6: NNInt exclusive flag support

**Files:**
- Modify: `src/storage/nnint.rs`
- Modify: `tests/storage.rs`

- [ ] **Step 1: Write failing test for exclusive propagation**

Add to `tests/storage.rs`:

```rust
contract_test!(test_exclusive_propagation {
    let data_keypath = Keypath::new();
    {
        let data: Data = DefaultFromKeypath::default_from_keypath(&data_keypath);
        data.store_keypath(&data_keypath);
    }

    let slot: u32 = 7;

    // Use at_mut_exclusive — should not panic, exclusive flag reaches NNInt
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        data.map.at_mut_exclusive(&slot).strict_inc(10);
    }

    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let v = data.map.at(&slot).get();
        assert_eq!(v, 10);
    }
});
```

- [ ] **Step 2: Run to verify it passes (exclusive currently ignored, just uses non-exclusive path)**

Run: `cargo test test_exclusive_propagation`
Expected: PASS (currently exclusive flag is set but not read by NNInt)

- [ ] **Step 3: Update NNInt methods to check exclusive flag**

Replace the method bodies in `src/storage/nnint.rs`. The keypath field is used to generate keys AND to check exclusive:

```rust
impl NNInt {
    pub fn new(keypath: &Keypath) -> Self {
        Self {
            keypath: keypath.clone(),
        }
    }

    /// Get the current value.  Aborts if location not yet set.
    pub fn get(&self) -> i128 {
        non_negative_int_get(&self.keypath.generate_key())
    }

    /// Get the current value, or none if not yet set
    pub fn try_get(&self) -> Option<i128> {
        non_negative_int_try_get(&self.keypath.generate_key())
    }

    /// Set the value
    pub fn set(&mut self, set_value: i128) {
        if self.keypath.is_exclusive() {
            non_negative_int_set_exclusive(
                &self.keypath.generate_key(),
                set_value,
            );
        } else {
            non_negative_int_set(&self.keypath.generate_key(), set_value);
        }
    }

    /// Increment by an amount (without querying the current value).
    pub fn inc(&mut self, increment: i128) {
        non_negative_int_inc(&self.keypath.generate_key(), increment);
    }

    /// Increment by an amount, aborting in case of a negative result.
    pub fn strict_inc(&mut self, increment: i128) {
        non_negative_int_strict_inc(&self.keypath.generate_key(), increment);
    }

    /// Delete the value at this storage location.
    pub fn delete(&mut self) {
        let key = &self.keypath.generate_key();
        if self.keypath.is_exclusive() {
            crate::delete_exclusive(key);
        } else {
            crate::delete_shared(key);
        }
    }
}
```

Note: `set_exclusive()` on NNInt is removed. The old `set_exclusive()` method is no longer needed — callers use `at_mut_exclusive()` instead. The `inc()` and `strict_inc()` methods do NOT have exclusive variants in the syscall layer (they use `set_add` which is always concurrent-safe), so they remain unchanged.

- [ ] **Step 4: Remove deprecated `set_exclusive` method from NNInt**

Remove these lines from `src/storage/nnint.rs`:

```rust
    /// Set the value exclusively
    pub fn set_exclusive(&mut self, set_value: i128) {
        non_negative_int_set_exclusive(
            &self.keypath.generate_key(),
            set_value,
        );
    }
```

- [ ] **Step 5: Fix any compilation errors from removed `set_exclusive`**

Run: `cargo check`
If any code references `NNInt::set_exclusive()`, update it to use `at_mut_exclusive()` + `set()` instead. Check `tests/nnints.rs` and example contracts.

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/storage/nnint.rs
git commit -m "feat: NNInt reads exclusive flag from keypath, remove set_exclusive"
```

---

### Task 7: Bytestring exclusive flag support

**Files:**
- Modify: `src/storage/bytestring.rs`

- [ ] **Step 1: Update Bytestring set() to check exclusive flag**

Replace the `set` and `set_exclusive` methods in `src/storage/bytestring.rs`:

```rust
    pub fn set(&mut self, buf: &[u8]) {
        if self.key.keypath().is_exclusive() {
            bytestring_set_exclusive(&self.key, buf);
        } else {
            bytestring_set(&self.key, buf);
        }
    }

    pub fn delete(&mut self) {
        let key = self.key.get();
        if self.key.keypath().is_exclusive() {
            crate::delete_exclusive(key);
        } else {
            crate::delete_shared(key);
        }
    }
```

Note: `bytestring_set_exclusive` is in `crate::sys_exn::bytestring` — check the import. If it's in `sys_result`, add the appropriate import.

- [ ] **Step 2: Remove deprecated `set_exclusive` method**

Remove:

```rust
    pub fn set_exclusive(&mut self, buf: &[u8]) {
        bytestring_set_exclusive(&self.key, buf);
    }
```

- [ ] **Step 3: Add the `delete` method if not present, add `bytestring_set_exclusive` import**

Check imports at top of file. The `bytestring_set_exclusive` function may be in `crate::sys_result::bytestring` rather than `crate::sys_exn::bytestring`. Add the appropriate import.

- [ ] **Step 4: Fix compilation**

Run: `cargo check`
Fix any references to `Bytestring::set_exclusive()`.

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/storage/bytestring.rs
git commit -m "feat: Bytestring reads exclusive flag from keypath, remove set_exclusive"
```

---

### Task 8: Migrate tests to new API

**Files:**
- Modify: `tests/storage.rs`

- [ ] **Step 1: Update `test_map` to use at()/at_mut()**

Replace the existing `test_map` test:

```rust
contract_test!(test_map {
    let data_keypath = Keypath::new();
    {
        let data: Data = DefaultFromKeypath::default_from_keypath(&data_keypath);
        data.store_keypath(&data_keypath);
    }

    let slot: u32 = 3;
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();

        // Use new at_mut() API
        data.map.at_mut(&slot).strict_inc(7);
    }

    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let v = data.map.at(&slot).get();
        assert_eq!(v, 7);
    }
});
```

- [ ] **Step 2: Update `test_map_entry` to use at_mut() + Value\<T\>**

The existing `test_map_entry` uses `Map<u32, i32>` with `entry()`, `set()`, and `try_get()`. Since `i32` doesn't implement `StorageNode`, this test should use the old API until we either implement StorageNode for primitives or switch to `Map<u32, Value<i32>>`. For now, leave this test using the old API — it will be updated when we remove the deprecated methods.

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add tests/storage.rs
git commit -m "test: migrate map tests to at()/at_mut() API"
```

---

### Task 9: Migrate ERC20 example contract

**Files:**
- Modify: `example-contracts/erc20/src/implementation.rs`

- [ ] **Step 1: Update `_balance_amount_of`**

```rust
fn _balance_amount_of(&self, owner: &ContractAddress) -> Amount {
    self.balances.at(owner).try_get().unwrap_or(0) as Amount
}
```

- [ ] **Step 2: Update `_adjust_balance`**

```rust
fn _adjust_balance(&mut self, owner: &ContractAddress, diff: Difference) {
    self.balances.at_mut(owner).strict_inc(diff);
}
```

- [ ] **Step 3: Update `_allowance`**

```rust
fn _allowance(
    &self,
    owner: &ContractAddress,
    spender: &ContractAddress,
) -> Amount {
    self.allowances
        .at(owner)
        .at(spender)
        .try_get()
        .unwrap_or(0) as Amount
}
```

- [ ] **Step 4: Update `_adjust_allowance`**

```rust
fn _adjust_allowance(
    &mut self,
    owner: &ContractAddress,
    spender: &ContractAddress,
    diff: Difference,
) {
    self.allowances
        .at_mut(owner)
        .at_mut(spender)
        .strict_inc(diff);
    log_approval_event(owner, spender, diff);
}
```

- [ ] **Step 5: Update `_adjust_total_supply`**

This uses `self.total_supply` directly (not through a Map), so it stays unchanged:

```rust
fn _adjust_total_supply(&mut self, diff: Difference) {
    self.total_supply.strict_inc(diff);
}
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p erc20`
Expected: PASS

- [ ] **Step 7: Run ERC20 tests**

Run: `cargo test -p erc20-test`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add example-contracts/erc20/src/implementation.rs
git commit -m "refactor: migrate ERC20 to at()/at_mut() API"
```

---

### Task 10: Remove deprecated Map methods and Entry type

**Files:**
- Modify: `src/storage/map.rs`

This is the breaking change. Only do this after all tests and examples are migrated.

- [ ] **Step 1: Remove Map::get() and Map::try_get()**

Remove these methods from `impl<K, V> Map<K, V>`:

```rust
    pub fn get(&self, entry_key: &K) -> V
    pub fn try_get(&self, entry_key: &K) -> Option<V>
```

- [ ] **Step 2: Remove Map::set() and Map::set_exclusive()**

Remove:

```rust
    pub fn set(&mut self, entry_key: &K, value: &V)
    pub fn set_exclusive(&mut self, entry_key: &K, value: &V)
```

- [ ] **Step 3: Remove Map::delete(), Map::delete_exclusive()**

Remove:

```rust
    pub fn delete(&mut self, entry_key: &K)
    pub fn delete_exclusive(&mut self, entry_key: &K)
```

- [ ] **Step 4: Remove Map::entry() and the entire Entry type**

Remove the `entry()` method and the `Entry` struct plus all its impls and the `Drop` impl (lines 105-262 in the original file).

- [ ] **Step 5: Fix compilation errors**

Run: `cargo check`
Expected: Errors in tests and examples that still use old API. Fix each one:
- `tests/storage.rs`: update `test_map_entry` to use `Map<u32, Value<i32>>` with `at_mut().set()` or remove if redundant with new tests
- Any other test using old methods

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/storage/map.rs tests/storage.rs
git commit -m "refactor: remove deprecated Map methods (get/set/entry/delete)"
```

---

## Deferred Work

The following items from the spec are intentionally deferred from this plan:

- **Handle\<T\> → Value\<T\> full migration:** Handle<T> continues to work alongside Value<T>. A follow-up should deprecate Handle<T>, alias it to Value<T>, and migrate remaining Handle usages (e.g., in Vector::get_handle()).
- **Vector at()/at_mut():** Vector should adopt the same at()/at_mut() pattern as Map, replacing get()/load()/load_mut(). Deferred per user direction.
- **Proc macro updates:** The `#[storage]` derive may need to generate `StorageNode` impls if user structs are used as Map values. Currently only built-in types need StorageNode, so this is deferred until needed.
