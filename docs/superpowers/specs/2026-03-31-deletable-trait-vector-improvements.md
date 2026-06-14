# Deletable Trait & Vector Improvements

## Problem

The storage layer has no generic mechanism for deleting storage data. Each leaf type (NNInt, Value<T>, Bytestring) has its own inherent `delete()` method, but there's no trait-level abstraction. This means:

- `Vector::pop()` can't exist generically — there's no way to delete an element of type `S: StorageType`.
- Compound types (Vector, Node) can't express recursive deletion.
- The exclusive locking flag doesn't propagate cleanly through deletion.

Additionally, Vector uses `NNInt` for its length, but since it already requires exclusive locking on the length field, a plain `Value<u32>` is sufficient and simpler.

## Design

### Deletable Trait

A new trait in `src/storage/deletable.rs`:

```rust
pub trait Deletable {
    /// Delete storage for this node. Flags are OR'd with the keypath's
    /// own exclusive flag by each implementor.
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

`delete()` and `delete_exclusive()` are default methods. Implementors only write `delete_internal`.

The `flags` parameter is OR'd with the keypath's own exclusive flag inside each implementation. This means exclusivity is additive:
- `delete()` on a slot whose keypath is exclusive → exclusive delete.
- `delete_exclusive()` → always exclusive, regardless of keypath.
- `delete()` on a slot whose keypath is not exclusive → shared delete.

### Flag Computation

Each leaf implementation computes effective flags as:

```rust
fn delete_internal(&mut self, flags: StorageDeltaFlags) {
    let effective = flags | self.keypath_flags();
    // perform delete with effective flags
}
```

Where `keypath_flags()` returns `StorageDeltaFlags::ExclusiveLock` if `keypath.is_exclusive()`, else `StorageDeltaFlags::None`. This matches how `NNInt::flags()` already works.

### Leaf Implementations

**NNInt**: Deletes the storage key using the combined flags. Replaces the existing inherent `delete()` method.

**Value\<T\>**: Deletes the storage key using the combined flags. Replaces the existing inherent `delete()` method.

**Bytestring**: Deletes the storage key using the combined flags. Replaces the existing inherent `delete()` method.

All three types currently have inherent `delete(&mut self)` methods that check `self.keypath.is_exclusive()` and dispatch to `delete_shared` or `delete_exclusive`. These are removed. The trait is the only delete API.

The underlying syscall is `raw_DeleteObject(key, flags)`, which already accepts flags. Add a `delete_with_flags(key, flags)` function to both `sys_result::delete` and `sys_exn::delete` that takes `StorageDeltaFlags` directly, paralleling how `nnint_internal::set_add` already takes flags. The existing `delete_shared`/`delete_exclusive`/`delete(key, bool)` functions remain for backward compatibility.

### Compound Implementations

**Vector\<S: StorageType + Deletable\>**: Iterates all elements (0..len), constructs each via `S::from(keypath)`, calls `delete_internal(flags)` on each, then calls `delete_internal(flags)` on the length field.

**Node\<T: Deletable\>**: Delegates to `self.fields.delete_internal(flags)`.

**Map**: Does NOT implement Deletable. Maps cannot enumerate their entries.

### Derive Macro

`#[derive(Storage)]` generates a conditional `Deletable` implementation:

```rust
impl Deletable for MyStruct
where
    FieldType1: Deletable,
    FieldType2: Deletable,
    ...
{
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        self.field1.delete_internal(flags);
        self.field2.delete_internal(flags);
        ...
    }
}
```

The where clause means the impl only takes effect when all fields are Deletable. A struct with a Map field won't be Deletable — no compile error unless code tries to use it as one. Same pattern as `#[derive(Clone)]`.

### Vector::pop()

Available when `S: StorageType + Deletable`:

```rust
pub fn pop(&mut self) {
    let len = self.len();
    if len == 0 {
        oob_throw(0, 0);
    }
    let new_len = len - 1;
    let mut elem = S::from(self.make_elem_keypath(new_len));
    elem.delete_internal(StorageDeltaFlags::ExclusiveLock);
    self.len.set(&new_len);
}
```

Pop deletes the last element with an exclusive lock, then decrements the length. Exclusive locking on the element delete matches the exclusivity of the length mutation (the length keypath has `set_exclusive()`), ensuring both operations have the same concurrency constraint.

### Vector Length: NNInt → Value\<u32\>

Vector's `len` field changes from `NNInt` to `Value<u32>`:
- The length keypath already has `set_exclusive()`, so Value's `set` method automatically uses exclusive locking.
- `NNInt` is unnecessary here — there's no need for concurrent increment semantics on a vector length. The exclusive lock means only one transaction can modify length per block anyway.
- `u32` limits vectors to ~4 billion elements, which is sufficient.

### Vector Index Type: u64 → u32

All Vector public API methods change index parameters from `u64` to `u32`:
- `len() -> u32`
- `at(index: u32)`
- `at_mut(index: u32)`
- `push() -> SlotMut`
- `pop()`

The internal `Map<u64, S>` changes to `Map<u32, S>` accordingly.

`bounds_check` casts to `u64` for the `oob_throw` call (which takes `u64, u64`).

`push` uses `checked_add(1)` to panic on overflow rather than silently wrapping.

## Scope

In scope:
- New `Deletable` trait with `delete()`, `delete_exclusive()`, `delete_internal(flags)`
- Leaf impls for NNInt, Value\<T\>, Bytestring
- Compound impls for Vector\<S: Deletable\> and Node\<T: Deletable\>
- Derive macro generates conditional Deletable impl
- Remove inherent `delete()` methods from leaf types
- Add `delete_with_flags(key, flags)` to sys_exn/sys_result delete modules
- `Vector::pop()`
- Vector length: NNInt → Value\<u32\>
- Vector index type: u64 → u32
- Update all callers (tests, examples)

Out of scope:
- Deletable for Map (would require iteration support)
- `try_pop()` / `Option`-returning pop variant
- Generic shrink/truncate on Vector
