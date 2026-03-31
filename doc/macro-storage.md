Derive traits necessary for manipulating data in the key-value store.

The [`storage`](macro@crate::storage) attribute-macro decorates the item
with `#[derive(Storage)]`.

Deriving `Storage` provides implementations of
[ReadXDRPath](crate::storage::ReadXDRPath),
[LoadKeypath](crate::storage::LoadKeypath) and
[FromKeypath](crate::storage::FromKeypath).

All fields in a `#[storage]` struct must be storage node types
(`Value<T>`, `NNInt`, `Bytestring`, `Map`, `Vector`). Plain data
must be wrapped in `Value<T>` to make the persistence model explicit.

Usage **examples** are in the top-level [documentation](mod@crate).
