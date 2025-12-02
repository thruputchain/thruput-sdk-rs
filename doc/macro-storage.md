Derive traits necessary for manipulating data in the key-value store.

The [`storage`](macro@crate::storage) attribute-macro simply decorates the item
with `#[derive(WriteXDR, Storage)]`.

Deriving `Storage`, in turn, provides implementations of
[ReadXDRPath](crate::storage::ReadXDRPath),
[LoadKeypath](crate::storage::LoadKeypath) and
[DefaultFromKeypath](crate::storage::DefaultFromKeypath).

