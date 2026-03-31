Derive traits necessary for manipulating data in the key-value store.

The [`storage`](macro@crate::storage) attribute-macro decorates the item
with `#[derive(Storage)]`.

Deriving `Storage` provides an implementation of
[`From<Keypath>`](crate::storage::Keypath).

All fields in a `#[storage]` struct must be storage types
(`Value<T>`, `NNInt`, `Bytestring`, `Map`, `Vector`, `Node<T>`).
Plain data must be wrapped in `Value<T>` to make the persistence
model explicit. Compound field groups can be wrapped in
[`Node<T>`](crate::storage::Node).

Usage **examples** are in the top-level [documentation](mod@crate).
