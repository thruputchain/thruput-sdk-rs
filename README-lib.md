This SDK facilitates writing Groundhog contracts in Rust.

<!--
Commonly-useful low-level items are available from
the [prelude].  Alternatively, a higher-level storage interface
is provided by [storage_prelude].
-->

# Prelude

The [`prelude`] is a low-level interface to mechanisms pertinent to
contract execution.
It includes primitive datatypes for working with contracts, like
[`ContractAddress`](prelude::ContractAddress),
[`StorageKey`](prelude::StorageKey) and
[`PublicKey`](prelude::PublicKey)
(which are also available directly from the module [`types`]).

The [`prelude`] also gives access to low-level routines for manipulating
the persistent contract state. These may also be
found in their own modules: [`bytestring`], [`nonnegative_int`],
[`retrocausal_queue`], and [`delete`].

# Allocation

If feature `alloc` is enabled:
*   The [`alloc`] crate is linked in, giving access to allocating
    datatypes like [`String`] and [`Vec`]
    from the prelude.  Direct access to heap allocation is available from
    module [`boxed`](alloc::boxed).
*   A simple "bump-allocator" is installed for heap allocation.  It never
    actually frees allocations until the contract invocation terminates.

Datatypes defined by third-party libraries or by the user may be
used in memory and in persistent storage, noting that:
*   Heap allocation requires feature `alloc`.
*   A datatype must implement the [`serde`](https://docs.rs/serde/latest/serde/) traits
    [`Serialize`](prelude::Serialize) and
    [`Deserialize`](prelude::Deserialize)
    (available from the [`prelude`] and also in
    module [`serialize`]) in order to be placed in persistent storage.

# Storage Prelude

The [`storage_prelude`], used in place of the lower-level [`prelude`],
provides a high-level layer for manipulating persistent storage, and
includes collection datatypes like
[`Map`](storage_prelude::Map)
that are implemented atop the VM's key-value storage.
