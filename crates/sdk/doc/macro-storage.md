Storage macros and traits for the key-value store.

# `#[storage]` / `#[derive(Storage)]`

The [`storage`](macro@crate::storage) attribute macro is shorthand for
`#[derive(Storage)]`.

Deriving `Storage` generates `From<Keypath>` for the struct, assigning a
sub-keypath to each field.

All fields must be SDK storage types. Plain data must be wrapped in
[`Value<T>`](crate::storage::Value) to make the persistence model explicit.

# Storage types

| Type | Description |
|------|-------------|
| [`Value<T>`](crate::storage::Value) | Single XDR-serializable value |
| [`Balance`](crate::storage::Balance) | Non-negative, zero-defaulting `u128` amount with checked and lazy `increase`/`decrease`; the type to reach for first for balances, allowances, and supply |
| [`NNInt`](crate::storage::NNInt) | `i128` counter whose signed increments merge across concurrent transactions; the floor is enforced at commit (see its docs for the `set`/`inc`/`strict_inc` rules) |
| [`CapacityConstraint`](crate::storage::CapacityConstraint) | Block-wide linear limit that concurrent transactions consume from |
| [`Bytestring`](crate::storage::Bytestring) | Variable-length byte blob |
| [`RCQ<T>`](crate::storage::RCQ) | Retrocausal queue: inserts commute, consumes are sequential |
| [`Map<K, V>`](crate::storage::Map) | Key-addressed collection (entries not enumerable) |
| [`Vector<S>`](crate::storage::Vector) | Index-addressed, sparse-capable sequence |
| [`Node<T>`](crate::storage::Node) | Nested storage struct used inside collections |

# Common patterns

### Single values

Use [`Value<T>`](crate::storage::Value) for ordinary stored fields.

`Value::set()` writes immediately to the storage delta for the current frame.

`Value::load_mut()` returns an auto-flushing edit guard: the updated value is
written back when the guard is dropped.

### Maps

[`Map<K, V>`](crate::storage::Map) stores independently addressed entries.
Entries may be absent; `at()` and `at_mut()` return storage-slot guards, not
references into an in-memory collection.

### Vectors

[`Vector<S>`](crate::storage::Vector) tracks an in-bounds extent with
[`Vector::len()`](crate::storage::Vector::len). Individual slots may still be
empty if their backing storage has been deleted, so vectors are sparse-capable.

Preferred append path:
- [`Vector::push_with()`](crate::storage::Vector::push_with) constructs the new
  slot, lets you initialize it, then increments len.

Low-level append path:
- [`Vector::push_empty_slot()`](crate::storage::Vector::push_empty_slot)
  increments len immediately and returns the new slot handle.

### Nested structs in collections

Collections store storage types, not plain structs. For a nested storage struct
inside a `Map` or `Vector`, wrap it in [`Node<T>`](crate::storage::Node):

```rust,ignore
#[derive(Storage)]
struct Inner {
    amount: Value<u64>,
}

#[derive(Storage)]
struct State {
    inners: Vector<Node<Inner>>,
}
```

# `Deletable` trait

[`Deletable`](crate::storage::Deletable) marks storage types whose data can be
fully deleted. It provides three methods:

- **`delete(&mut self)`**: delete using the keypath's own locking semantics
- **`delete_exclusive(&mut self)`**: delete with an exclusive lock
- **`delete_internal(&mut self, exclusive: bool)`**: low-level propagation
  hook used by derived impls

### Which types are `Deletable`?

| Type | Deletable? | Notes |
|------|-----------|-------|
| `NNInt` | Yes | |
| `Balance` | Yes | |
| `CapacityConstraint` | Yes | |
| `Value<T>` | Yes | |
| `Bytestring` | Yes | |
| `RCQ<T>` | Yes | Deletion cost scales with the queue's size |
| `Node<T>` | When `T: Deletable` | |
| `Vector<S>` | When `S: Deletable` | |
| `Map<K, V>` | **No** | Cannot enumerate entries |

# `#[derive(Deletable)]`

Opt-in derive macro. Generates a `Deletable` impl that calls
`delete_internal()` on every field.

If any field's type does not implement `Deletable`, the derive produces a
compile error at the struct definition, similar to deriving `Clone` for a
struct containing a non-`Clone` field.

```rust,ignore
#[storage]
#[derive(Deletable)]
struct Counters {
    a: NNInt,
    b: Value<u64>,
}
// Counters::delete() will delete both `a` and `b`.
```

Structs containing `Map` fields cannot derive `Deletable`:

```rust,ignore
#[storage]
struct Registry {
    entries: Map<Address, Value<u64>>,
    total: NNInt,
}
// #[derive(Deletable)] on Registry would fail:
// Map<_, _> does not implement Deletable.
```

Usage examples are in the top-level [documentation](mod@crate).
