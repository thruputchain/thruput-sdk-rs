# Unified Storage Slot API

## Problem

The current Map API has several interrelated issues:

1. **Double-borrow footgun on NNInt**: `Map::get()` returns NNInt by value. Two independent handles to the same storage slot can coexist and issue conflicting syscalls (TOCTOU via `try_get` then `set_add` in `strict_inc`).

2. **Inconsistent persistence models**: NNInt and Bytestring mutations are immediate syscalls. `Entry` uses flush-on-drop. Value types returned by `Map::get()` are disconnected copies that silently don't persist.

3. **Broken delete on composites**: `Map::delete()` on `Map<K, Map<K2, V>>` only removes the outer entry, orphaning inner entries (no range delete support).

4. **API surface duplication**: `Map::set()`, `Map::entry()`, and direct mutation through handles returned by `Map::get()` are three different persistence paths with different semantics.

5. **Exclusive lock granularity**: `set_exclusive()` exists per-operation on individual types, but exclusivity is fundamentally a per-slot decision (each slot becomes one DB write with the flag on or off).

## Design Assumptions

- **Reentrancy is banned.** This means lazy persistence (flush-on-drop) and eager persistence (immediate syscall) are not observably different, as long as borrow guards prevent aliased access.

## Core Principle

**Collections are slot factories, not value loaders.** Maps and Vectors produce borrowed handles to storage slots. All reads and writes happen through the slot's own methods. The guard enforces Rust-style shared/exclusive access at the collection level. Persistence is each storage type's own concern — the slot system is purely about borrow checking.

## StorageNode Trait

All storage types implement a single unifying trait:

```rust
pub trait StorageNode: Sized {
    fn from_keypath(keypath: Keypath) -> Self;
    fn keypath(&self) -> &Keypath;
}
```

Implemented by: NNInt, Bytestring, Value<T>, Map<K, S>, Vector<S>.

## Guard Types

Two guard types enforce borrow semantics on collections. Both deref to the StorageNode itself — no associated target type, no eager deserialization, no flush-on-drop.

```rust
/// Read-only borrowed slot. Holds a shared borrow on the parent collection.
pub struct SlotRef<'a, Owner, S: StorageNode> {
    slot: S,
    _borrow: PhantomData<&'a Owner>,
}

impl<Owner, S: StorageNode> Deref for SlotRef<'_, Owner, S> {
    type Target = S;
    fn deref(&self) -> &S { &self.slot }
}
```

```rust
/// Mutable borrowed slot. Holds an exclusive borrow on the parent collection.
pub struct SlotMut<'a, Owner, S: StorageNode> {
    slot: S,
    _borrow: PhantomData<&'a mut Owner>,
}

impl<Owner, S: StorageNode> Deref for SlotMut<'_, Owner, S> {
    type Target = S;
    fn deref(&self) -> &S { &self.slot }
}

impl<Owner, S: StorageNode> DerefMut for SlotMut<'_, Owner, S> {
    fn deref_mut(&mut self) -> &mut S { &mut self.slot }
}

// No Drop impl. SlotMut is a pure borrow guard.
```

**Borrow semantics:**
- Multiple `SlotRef`s can coexist (shared borrow on parent).
- A `SlotMut` prevents any other access to the parent while alive (exclusive borrow).
- A `SlotRef` and `SlotMut` on the same collection cannot coexist.

**Why no `SlotFlush` trait:** SlotMut never holds deserialized data — it holds the StorageNode handle. Since it never loads data, it never needs to flush. Flush-on-drop is the responsibility of `Value<T>::load_mut()` (see below), not the slot system. This keeps the slot layer purely about borrow checking.

## Storage Types

### NNInt

Handle type. All operations are immediate syscalls.

```rust
pub struct NNInt { keypath: Keypath }

impl NNInt {
    pub fn get(&self) -> i128;
    pub fn try_get(&self) -> Option<i128>;
    pub fn set(&mut self, value: i128);
    pub fn inc(&mut self, delta: i128);
    pub fn strict_inc(&mut self, delta: i128);
    pub fn delete(&mut self);
}
```

The `&self`/`&mut self` split aligns with SlotRef (read-only) and SlotMut (read-write) access. Through `SlotRef`, only `get()` and `try_get()` are callable. Through `SlotMut`, mutation methods become available via `DerefMut`.

The per-operation `set_exclusive()` and `delete_exclusive()` methods are removed. Exclusivity is determined at slot acquisition time (see Exclusive Locks below) and propagated to syscalls internally.

### Bytestring

Handle type. All operations are immediate syscalls.

```rust
pub struct Bytestring { key: KeyCache }

impl Bytestring {
    pub fn get(&self) -> Box<[u8]>;
    pub fn try_get(&self) -> Option<Box<[u8]>>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn set(&mut self, buf: &[u8]);
    pub fn delete(&mut self);
}
```

Same `&self`/`&mut self` split as NNInt. `set_exclusive()` removed; exclusivity via slot acquisition.

### Value\<T\> (replaces Handle\<T\>)

Wrapper for serialized value types in storage.

```rust
pub struct Value<T> {
    key: KeyCache,
    _marker: PhantomData<fn() -> T>,
}
```

Value<T> exposes the same read/write pattern as NNInt and Bytestring — immediate syscalls, no eager deserialization:

```rust
impl<T: ReadXDROwned + WriteXDR> Value<T> {
    pub fn get(&self) -> T;          // deserialize and return
    pub fn try_get(&self) -> Option<T>;  // None if key doesn't exist
    pub fn set(&mut self, value: &T);    // serialize and store (immediate)
    pub fn delete(&mut self);
}
```

For in-place modification, `load_mut()` returns a guard that deserializes T, gives `&mut T` access, and flushes on drop:

```rust
impl<T: ReadXDROwned + WriteXDR> Value<T> {
    pub fn load_mut(&mut self) -> ValueMut<'_, T>;
}

pub struct ValueMut<'a, T> {
    value: &'a mut Value<T>,
    data: T,
}

impl<T> Deref for ValueMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &T { &self.data }
}

impl<T> DerefMut for ValueMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T { &mut self.data }
}

impl<T: WriteXDR> Drop for ValueMut<'_, T> {
    fn drop(&mut self) {
        // Flushes via Value<T>::set(), which is an immediate syscall.
        // The exclusive flag on the KeyCache determines store vs store_exclusive.
        self.value.set(&self.data);
    }
}
```

**Why flush-on-drop lives here, not on SlotMut:** SlotMut never loads data — it wraps the Value<T> handle and provides `&mut Value<T>`. Only when the caller explicitly calls `load_mut()` does deserialization happen. If the caller uses `set()` or `delete()` directly instead, no load occurs and no flush is needed. This makes the cost explicit and avoids unnecessary syscalls.

`HandleRef` and `HandleRefMut` are removed; `SlotRef`/`SlotMut` replace the borrow guard role, and `ValueMut` replaces the flush-on-drop role.

### Map\<K, S\>

Collection type. Produces borrowed slots via `at()`/`at_mut()`.

```rust
impl<K: WriteXDR, S: StorageNode> Map<K, S> {
    pub fn at(&self, key: &K) -> SlotRef<'_, Self, S>;
    pub fn at_mut(&mut self, key: &K) -> SlotMut<'_, Self, S>;
    pub fn at_mut_exclusive(&mut self, key: &K) -> SlotMut<'_, Self, S>;
}
```

`at()`/`at_mut()` construct a StorageNode from the derived keypath. For handle types (NNInt, Bytestring, nested Map), this is just keypath construction — always succeeds, no syscall. For Value<T>, same — the Value handle is constructed from the keypath without loading data. Existence is checked through the slot's own `try_get()`.

**Removed methods:** `get()`, `try_get()`, `set()`, `set_exclusive()`, `entry()`.

**`delete()` is not on Map.** It is on the individual leaf storage types (NNInt, Bytestring, Value<T>) that occupy a single storage key. Composite types (Map, Vector) do not expose delete because removing the collection handle does not remove its entries. To delete a leaf through a Map: `map.at_mut(key).delete()`.

## Exclusive Locks

Exclusivity is a per-slot decision, made at acquisition time:

```rust
// Concurrent writes (default) — suitable for commutative operations like inc
let mut slot = map.at_mut(key);

// Exclusive write — prevents concurrent modification of this slot
let mut slot = map.at_mut_exclusive(key);
```

### Propagation through nested collections

**The exclusive flag lives on the Keypath/KeyCache.** When a parent constructs a child keypath (e.g., inside `at_mut()`), the child inherits the parent's exclusive flag. This means exclusivity propagates automatically through nested collections:

```rust
// Exclusive set at outermost level, propagates to leaf:
self.allowances
    .at_mut_exclusive(owner)  // marks keypath exclusive
    .at_mut(spender)          // child inherits exclusive from parent
    .strict_inc(diff);        // syscall uses exclusive variant

// No need to repeat _exclusive at each level.
// Equivalent to the above — inner at_mut_exclusive is redundant but harmless:
self.allowances
    .at_mut_exclusive(owner)
    .at_mut_exclusive(spender)
    .strict_inc(diff);
```

### How it reaches syscalls

- **Handle types (NNInt, Bytestring):** Syscall methods read the exclusive flag from their key/keycache and dispatch to the appropriate variant (e.g., `non_negative_int_set` vs `non_negative_int_set_exclusive`).
- **Value\<T\>:** `set()` and `ValueMut`'s flush-on-drop read the exclusive flag from the KeyCache to choose `store()` vs `store_exclusive()`.

Individual `_exclusive` method variants (e.g., `NNInt::set_exclusive()`, `Bytestring::set_exclusive()`, `Map::delete_exclusive()`) are removed.

## Contract Struct Fields

Top-level storage fields use explicit storage node types:

```rust
#[storage]
pub struct TokenContract {
    contract_owner: Value<ContractAddress>,
    balances: Map<ContractAddress, NNInt>,
    allowances: Map<ContractAddress, Map<ContractAddress, NNInt>>,
    total_supply: NNInt,
}
```

`Value<T>` is explicit in the type — it makes the storage-backed nature visible. The `#[storage]` derive generates `StorageNode` implementations.

## ERC20 Migration Example

### Before

```rust
fn _balance_amount_of(&self, owner: &ContractAddress) -> Amount {
    let balance_cell = self.balances.get(owner);
    balance_cell.try_get().unwrap_or(0) as Amount
}

fn _adjust_balance(&mut self, owner: &ContractAddress, diff: Difference) {
    let mut balance = self.balances.get(owner);
    balance.strict_inc(diff)
}

fn _allowance(&self, owner: &ContractAddress, spender: &ContractAddress) -> Amount {
    let owner_allowances = self.allowances.get(owner);
    let spender_allowance = owner_allowances.get(spender);
    spender_allowance.try_get().unwrap_or(0) as Amount
}

fn _adjust_allowance(&mut self, owner: &ContractAddress, spender: &ContractAddress, diff: Difference) {
    let owner_allowances = self.allowances.get(owner);
    let mut spender_allowance = owner_allowances.get(spender);
    spender_allowance.strict_inc(diff);
}
```

### After

```rust
fn _balance_amount_of(&self, owner: &ContractAddress) -> Amount {
    self.balances.at(owner).try_get().unwrap_or(0) as Amount
}

fn _adjust_balance(&mut self, owner: &ContractAddress, diff: Difference) {
    self.balances.at_mut(owner).strict_inc(diff);
}

fn _allowance(&self, owner: &ContractAddress, spender: &ContractAddress) -> Amount {
    self.allowances.at(owner).at(spender).try_get().unwrap_or(0) as Amount
}

fn _adjust_allowance(&mut self, owner: &ContractAddress, spender: &ContractAddress, diff: Difference) {
    self.allowances.at_mut(owner).at_mut(spender).strict_inc(diff);
}
```

## Value\<T\> Usage Examples

### Immediate read/write (same pattern as NNInt)

```rust
fn get_owner(&self) -> ContractAddress {
    self.contract_owner.get()
}

fn set_owner(&mut self, owner: &ContractAddress) {
    self.contract_owner.set(owner);  // immediate syscall
}
```

### In-place modification via load_mut()

```rust
#[storage]
pub struct MyContract {
    profiles: Map<ContractAddress, Value<UserProfile>>,
}

impl MyContract {
    fn read_profile(&self, user: &ContractAddress) -> Option<UserProfile> {
        self.profiles.at(user).try_get()
    }

    fn update_profile_name(&mut self, user: &ContractAddress, name: String) {
        // load_mut() deserializes, gives &mut T, flushes on drop
        let mut profile = self.profiles.at_mut(user).load_mut();
        profile.display_name = name;
        // ValueMut drops → Value<T>::set() → immediate syscall
    }

    fn replace_profile(&mut self, user: &ContractAddress, profile: &UserProfile) {
        // set() is immediate — no load, no flush
        self.profiles.at_mut(user).set(profile);
    }
}
```

## Summary of Removed API Surface

| Removed | Replaced by |
|---------|------------|
| `Map::get()` / `Map::try_get()` | `Map::at()` + slot methods (`try_get()`, `get()`) |
| `Map::set()` / `Map::set_exclusive()` | `Map::at_mut()` + slot methods (`set()`, `load_mut()`) |
| `Map::delete()` / `Map::delete_exclusive()` | Slot-level `delete()` via `map.at_mut(key).delete()` |
| `Map::entry()` / `Entry` | `Map::at_mut()` + `Value<T>::load_mut()` for in-place modification |
| `Handle<T>` / `HandleRef` / `HandleRefMut` | `Value<T>` + `SlotRef` / `SlotMut` + `ValueMut` |
| `NNInt::set_exclusive()` | `Map::at_mut_exclusive()` (flag propagates to syscalls) |
| `Bytestring::set_exclusive()` | `Map::at_mut_exclusive()` (flag propagates to syscalls) |
