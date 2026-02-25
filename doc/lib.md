This SDK facilitates writing Groundhog contracts in Rust.

# System functions

The SDK provides convenient wrappers around Groundhog syscalls:
- [`deploy`]: Deploying other contracts
- [`mod@call`]: Calling other contracts
- [`contract_op`]: Contract entrance/exit routines
- [`crypto`]: Hash and sign
- [`Hasher`]: Incremental hashing
- Key-value storage:
    - [`mod@delete`]
    - [`mod@bytestring`]
    - [`mod@non_negative_int`]
    - [`mod@retrocausal_queue`]
- Low-level bindings:
    - [`mod@sys`]: Low-level wrappers

Other things:
- [`mod@types`]: Types related to system facilities, like:
    - [`ContractAddress`](ContractAddress)
    - [`StorageKey`](StorageKey)
    - [`PublicKey`](PublicKey)

# Allocation

### Static

Static allocations are facilitated in contracts by [`ContractCell`] and
[`contract_local!`].

### Heap

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

*TODO*: Rename this module

# Macros

The [`storage`](macro@crate::storage) macro derives traits necessary for
storing data in the key-value store.

The [`contract`](macro@crate::contract) macro exports a contract's methods for
responding to cross-contract calls, and also creates "client" objects for
performing those calls.

# Example contract
```
#[storage]
pub struct MyContract {
    contract_owner: ContractAddress,
}

pub struct MyError;

pub type Result<T, E = MyError> = Result<T, E>;

#[contract(exports = "guest_exports", err_type = MyError, client = MyClient)]
impl MyContract {
    #[init]
    pub fn initialize(&mut self, contract_owner: &ContractAddress) -> Result<()> {
        self.contract_owner = contract_owner.clone();
        Ok(())
    }

    pub fn get_owner(&self) -> Result<ContractAddress> {
        self.contract_owner.clone()
    }
}
```

