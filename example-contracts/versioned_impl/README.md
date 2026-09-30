# Importing an interface from a different versioned crate

This example consists of two crates:

- `../versioned_interface` is version **0.1.0** and declares `Adder`.
- `versioned_impl` is version **0.2.0** and implements the imported `Adder`.

Both use plain `#[contract]`, with no `version` argument. The trait takes its
version from its own crate's `Cargo.toml`; the implementation inherits it.
The generated `AdderClient` therefore calls the implementation using the
0.1 interface's entrypoint ID.

From the repository root, run the native test:

```sh
cargo test --manifest-path example-contracts/Cargo.toml -p versioned_impl --locked
```

To exercise the same call between two deployed RV32 contracts:

```sh
./scripts/dev.sh bottle-test "$(pwd)/example-contracts" versioned_impl
```

The regular example CI runs both tests.
