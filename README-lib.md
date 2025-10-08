This SDK facilitates writing Groundhog contracts in Rust.

# System functions

The SDK provides convenient wrappers around Groundhog syscalls:
- [`deploy`]: Deploying other contracts
- [`mod@invoke`]: Calling other contracts
- [`contract_op`]: Contract entrance/exit routines
- [`crypto`]: Hash and sign
- Key-value storage:
    - [`delete`]
    - [`bytestring`]
    - [`nonnegative_int`]
    - [`retrocausal_queue`]
- Low-level bindings:
    - [`SysErr`]: Error type for syscalls
    - [`sys`]: Low-level wrappers

Other things:
- [`types`]: Types related to system facilities, like:
    - [`ContractAddress`](ContractAddress)
    - [`StorageKey`](StorageKey)
    - [`PublicKey`](PublicKey)
- [`sysresult`]

# Allocation

If feature `alloc` is enabled:
*   The [`alloc`] crate is linked in, giving access to allocating
    datatypes like [`String`] and [`Vec`].  Direct access to heap allocation is
    available from module [`boxed`](alloc::boxed).
*   A simple "bump-allocator" ([`allocator`]) is installed for heap allocation.  It never
    actually frees allocations until the contract invocation terminates.

Datatypes defined by third-party libraries or by the user may be
used in memory and in persistent storage, noting that:
*   Heap allocation requires feature [`alloc`].
*   Datatypes should be decorated with the [`storage`](macro@storage) attribute-macro.

# Storage

The [`mod@storage`] module provides a high-level layer for manipulating persistent
storage, and includes collection datatypes like [`Map`](storage::Map) that are
implemented atop the VM's key-value storage.
