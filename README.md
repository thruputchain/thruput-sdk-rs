# Thruput Rust SDK

Write smart contracts in Rust for Thruput's RV32 runtime. The SDK provides
contract macros, generated clients, persistent storage types, typed exceptions,
events, and a native test environment.

## Start a contract

Use the `thruput` CLI to create and build a contract:

```sh
thruput new my-contract
cd my-contract
thruput build
```

See [Getting Started](https://docs.thruput.network/start/getting-started/) for
installing the CLI and setting up a network. The scaffold supplies the
manifest, contract library, RV32 binary wrapper, and pinned Rust toolchain.
Keep its `entrypoints` feature and `src/main.rs` wrapper: they link the
contract's entrypoints into the deployed artifact.

To work without the CLI, follow the
[Cargo-only guide](https://github.com/thruputchain/thruput-sdk-rs/blob/main/crates/sdk/doc/cargo-only.md).
It covers a standalone manifest, native tests, and an RV32 ELF build.

## Test a contract natively

Native tests run contracts in-process against the Thruput executor, with no
guest build. Enable them with the SDK's `dev` feature, and call
`thruput_sdk::declare_mock_contract!()` in the contract library to generate
`register_mock_contract`.

For a single transaction, `thruput_sdk::client_test` deploys a registered
contract and passes its generated client to a closure:

```rust
#[test]
fn increment_counter() {
    thruput_sdk::client_test(
        |env| counter::register_mock_contract(env),
        |client: &counter::CounterClient| {
            client.initialize(10);
            client.increment(5);
            assert_eq!(client.get(), 15);
        },
    );
}
```

Use `TestEnvironment::transact` for multiple transactions, multiple contracts,
or frame-level exception and fault assertions. The
[counter tests](https://github.com/thruputchain/thruput-sdk-rs/blob/main/fixtures/counter/tests/test.rs)
show deployment and error inspection, and the
[multi-transaction tests](https://github.com/thruputchain/thruput-sdk-rs/blob/main/fixtures/complex-storage/tests/test.rs)
show persistent state across transactions.

## Features

| Feature | Use |
| --- | --- |
| `alloc` | Heap allocation in guest builds |
| `std` | Required for host builds; forbidden on the guest |
| `dev` | The native test environment |
| `remote` | Off-chain proxies for calling deployed contracts |
| `logging` | Traces contract calls in native tests |

## Arithmetic

Contract release builds disable overflow checks. Integer operators, including
`U256` and `I256`, can wrap on-chain where a native test panics. Use checked
arithmetic for amounts: a passing native test does not establish guest
overflow behavior. The
[SDK guide](https://github.com/thruputchain/thruput-sdk-rs/blob/main/crates/sdk/doc/lib.md#arithmetic)
covers checked operations, full-precision multiplication and division, and
storage guards.

## Documentation

- The [SDK guide](https://github.com/thruputchain/thruput-sdk-rs/blob/main/crates/sdk/doc/lib.md)
  explains the generated code, features, storage, calls, and errors. It is
  also the crate-level rustdoc.
- [thruput-examples](https://github.com/thruputchain/thruput-examples) has
  worked contracts.
- `fixtures/` in this repository is the SDK's test corpus. Its
  fixtures exercise SDK behavior and are not audited application templates.

## Contributing

See [CONTRIBUTING.md](https://github.com/thruputchain/thruput-sdk-rs/blob/main/CONTRIBUTING.md)
for setting up this repository, building guest contracts, and running CI.

## License

Copyright 2026 Stellar Development Foundation. Licensed under the Apache
License, Version 2.0. Each published crate includes the workspace
[LICENSE](https://github.com/thruputchain/thruput-sdk-rs/blob/main/LICENSE).
