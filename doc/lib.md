This SDK facilitates writing Groundhog contracts in Rust.

# System functions

The SDK provides convenient wrappers around Groundhog syscalls:
- [`deploy`]: Deploying other contracts
- [`mod@call`]: Calling other contracts
- [`contract_op`]: Contract entrance/exit routines
- [`crypto`]: Hash and sign
- [`Hasher`]: Incremental hashing
- Key-value storage:
    - [`sys_exn::delete`]
    - [`sys_exn::bytestring`]
    - [`sys_exn::nnint`]
    - [`sys_exn::rcq`]
- Low-level syscall bindings:
    - [`mod@sys`]: Low-level wrappers
- Emitting events: [`mod@event`]

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

# Testing

**TODO:** Unify the below approaches into a single test-builder.

The SDK provides some utilities for executing contract code in the included
test environment.

This creates a cargo test named `test_bytestring`, containing the given code:
```
contract_test!(test_bytestring {
    let key = StorageKey::default();

    bytestring_set(&key, #b"");
});
```

To run test code against a client stub that calls your contract:

```
sdk::client_test(
    implementation::register,
    |c: &mut implementation::MyContractClient| {
        // Call methods on `c`
    });
```

A more flexible approach allows direct manipulation of the environment:
```
// Create a test environment where no storage-cost limits are imposed.
let env = TestEnvironment::no_cost();

// Execute some code in the environment.
contract_exec(env, || {
    let key = StorageKey::default();

    bytestring_set(&key, #b"");
})
.unwrap_or_else(|e| panic!("Test failed: {e}"));

```

A full list of test-running functions and macros that may be useful in various
circumstances:

- [`contract_exec()`]
- [`contract_test()`]
- [`contract_test_raw()`]
- [`contract_test_handler()`]
- [`contract_handler_exec()`]
- [`contract_handler_exec_raw()`]
- [`contract_test!`]
- [`client_test`]
- [`load_wasm_contract()`]
- [`test_wasm()`]

# Example contract

```
use sdk::*;

#[storage]
pub struct MyContract {
    contract_owner: ContractAddress,
}

#[derive(Debug, PartialEq, Error, ReadXDR, WriteXDR)]
#[exn]
#[error("something went wrong")]
pub struct MyError;

#[contract(throws = MyError, client = MyClient)]
impl MyContract {
    #[init]
    pub fn initialize(&mut self, contract_owner: &ContractAddress) {
        self.contract_owner = contract_owner.clone();
    }

    pub fn get_owner(&self) -> ContractAddress {
        self.contract_owner.clone()
    }

    pub fn oops(&self) {
        throw(MyError);
    }
}
```
