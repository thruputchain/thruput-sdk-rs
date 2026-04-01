Storage macros and traits for the key-value store.

# `#[storage]` / `#[derive(Storage)]`

The [`storage`](macro@crate::storage) attribute macro is shorthand for
`#[derive(Storage)]`. Deriving `Storage` generates an implementation of
[`From<Keypath>`](crate::storage::Keypath), which constructs the struct
from a keypath, assigning sub-keypaths to each field.

All fields must be storage types. Plain data must be wrapped in
[`Value<T>`](crate::storage::Value) to make the persistence model
explicit.

# Storage types

| Type | Description |
|------|-------------|
| [`Value<T>`](crate::storage::Value) | Single XDR-serializable value |
| [`NNInt`](crate::storage::NNInt) | Non-negative 64-bit integer with atomic arithmetic |
| [`Bytestring`](crate::storage::Bytestring) | Variable-length byte blob |
| [`Map<K, V>`](crate::storage::Map) | Key-addressed collection (entries not enumerable) |
| [`Vector<S>`](crate::storage::Vector) | Index-addressed, length-tracked sequence |
| [`Node<T>`](crate::storage::Node) | Nested sub-struct (T must derive Storage) |

# `Deletable` trait

[`Deletable`](crate::storage::Deletable) marks storage types whose data
can be fully deleted. It provides three methods:

- **`delete(&mut self)`** -- delete using the keypath's own locking
  semantics.
- **`delete_exclusive(&mut self)`** -- delete with an exclusive lock.
- **`delete_internal(&mut self, flags)`** -- low-level; used by derived
  impls to propagate flags.

### Which types are Deletable?

| Type | Deletable? | Notes |
|------|-----------|-------|
| `NNInt` | Yes | |
| `Value<T>` | Yes | |
| `Bytestring` | Yes | |
| `Node<T>` | When `T: Deletable` | |
| `Vector<S>` | When `S: Deletable` | |
| `Map<K, V>` | **No** | Cannot enumerate entries |

# `#[derive(Deletable)]`

Opt-in derive macro. Generates a `Deletable` impl that calls
`delete_internal` on every field. If any field's type does not implement
`Deletable`, the derive produces a compile error at the struct definition
-- the same behavior as `#[derive(Clone)]` with a non-Clone field.

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

Usage **examples** are in the top-level [documentation](mod@crate).
