# Two versions of the same interface crate in one contract

This example uses two releases of the **same package**, `versioned_interface`:

- `../versioned_interface` is **0.1.0**: `Adder::add_one(u32) -> u32`.
- `../versioned_interface_v2` is **0.2.0**: `Adder::add_one(u64) -> u64`.
- `versioned_impl` is **0.2.0** and implements both traits on `Contract`.

Its Cargo dependencies rename the two releases:

```toml
adder_v1 = { package = "versioned_interface", version = "=0.1.0", path = "../versioned_interface" }
adder_v2 = { package = "versioned_interface", version = "=0.2.0", path = "../versioned_interface_v2" }
```

Both traits and implementations use plain `#[contract]`, with no `version`
argument. Each trait takes its interface version from its own crate's
`Cargo.toml`. The two `Adder::add_one` entrypoints therefore have different
IDs even though their crate, trait, and method names are identical.

The native and RV32 tests use both clients at the **same deployed contract
address**, alternating calls between versions. The 0.2 call uses a value
larger than `u32::MAX` to exercise its different ABI.

Cargo requires unique package names among workspace members, so the 0.2
interface release is excluded from the parent workspace. It is still built
and tested as a dependency of `versioned_impl`; both versions appear in the
parent `Cargo.lock`.

From the repository root, run the native test:

```sh
cargo test --manifest-path example-contracts/Cargo.toml -p versioned_impl --locked
```

To exercise the same calls between two deployed RV32 contracts, build the
RV32 examples and run `versioned_impl_caller`'s tests. That contract calls
both interface versions from inside the guest:

```sh
cargo gnaw build versioned_impl versioned_impl_caller
cargo gnaw test versioned_impl_caller
```

The regular example CI runs both tests.
