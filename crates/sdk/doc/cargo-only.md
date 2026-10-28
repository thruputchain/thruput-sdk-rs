# Build a contract with Cargo

You can write and test a contract without the Thruput CLI. Cargo can also build
the RV32 ELF. Producing a deployable `.rv32a` requires a separate lowering step;
the CLI's `thruput build` performs that step. An ELF is not a deployed contract.

This example uses a local SDK clone and ordinary Cargo commands. It needs
Rust/rustup, a native linker, and registry access. Native tests also download the
test executor; see [native executor setup](../../native-exec/README.md).

## Create a standalone project

Start in the parent directory of your `thruput-sdk-rs` clone:

```sh
cargo new --lib hello-contract
cd hello-contract
mkdir -p .cargo tests
cp ../thruput-sdk-rs/rust-toolchain.toml .
```

Use the following files. If your clone has a different name or location, change
both SDK dependency paths in the manifest and the `--config` path in the build
command. Cargo does not inherit configuration from a path dependency's checkout.

### `Cargo.toml`

```toml
[package]
name = "hello-contract"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["rlib"]

[[bin]]
name = "hello_contract"
path = "src/main.rs"
required-features = ["entrypoints"]
test = false
bench = false

[features]
entrypoints = []
dev = ["thruput-sdk/dev"]

[target.'cfg(target_arch = "riscv32")'.dependencies]
thruput-sdk = { path = "../thruput-sdk-rs/crates/sdk", features = ["alloc"] }

[target.'cfg(not(target_arch = "riscv32"))'.dependencies]
thruput-sdk = { path = "../thruput-sdk-rs/crates/sdk", features = ["std"] }

[dev-dependencies]
hello-contract = { path = ".", features = ["dev"] }
```

`alloc` supports the owned strings used here. `std` is required on the host and
forbidden on the guest. The self-dependency enables the native executor during
tests. `entrypoints` includes the exported methods in the RV32 binary.

### `.cargo/config.toml`

```toml
[registries.thruput]
index = "sparse+https://crates.thruput.network/api/v1/crates/"
credential-provider = "cargo:token"
```

The SDK currently uses a private registry. Supply your access token through
`CARGO_REGISTRIES_THRUPUT_TOKEN` or Cargo's credential store. Do not put a token
in the project configuration or commit it.

### `src/lib.rs`

```rust
#![cfg_attr(not(feature = "dev"), no_std)]

extern crate alloc;

use alloc::string::String;
use thruput_sdk::*;

thruput_sdk::declare_mock_contract!();

pub struct Greeting;

#[contract]
impl Greeting {
    #[contract(effect = "readOnly")]
    pub fn greet(name: String) -> String {
        alloc::format!("Hello, {name}!")
    }
}
```

### `src/main.rs`

```rust
#![no_std]
#![no_main]

extern crate hello_contract;
```

This wrapper links the contract library and its generated entrypoints. The SDK
supplies the guest runtime and linker script.

### `tests/greeting.rs`

```rust
use hello_contract::GreetingClient;

#[test]
fn greets_through_the_generated_client() {
    thruput_sdk::client_test(
        |env| hello_contract::register_mock_contract(env),
        |client: &GreetingClient| {
            assert_eq!(client.greet(String::from("friend")), "Hello, friend!");
            assert_eq!(client.greet(String::from("🦀")), "Hello, 🦀!");
        },
    );
}
```

## Test and build

From `hello-contract/`, run the native test:

```sh
cargo test
```

Build the RV32 ELF using the SDK's maintained target configuration:

```sh
cargo --config ../thruput-sdk-rs/example-contracts/.cargo/config.toml build \
  --release --target riscv32im-unknown-none-elf \
  --no-default-features --features entrypoints --bin hello_contract \
  -Zbuild-std=core,alloc --target-dir target
```

The explicit `--target-dir` keeps this app's output in its own directory. The
target configuration sets the required linker and runtime flags; `build-std`
rebuilds `core` and `alloc` with those flags. Native tests do not need it. The
binary uses an underscored name to match the CLI build convention.

The output is `target/riscv32im-unknown-none-elf/release/hello_contract`, an ELF
intermediate containing entrypoints and raw specification data. It is not a
deployable `.rv32a`, and passing native tests does not prove guest execution.
To produce the lowered artifact and finalized specs, use a compatible Thruput
CLI; see [Build guest contracts](../../../CONTRIBUTING.md#build-guest-contracts).

The repository's `scripts/cargo-example-smoke.sh` builds these exact file
examples in a temporary standalone project. SDK CI runs this check so the guide
stays aligned with the code.

For storage and tests across transactions, continue with the executable examples
linked from [Test a contract natively](../../../README.md#test-a-contract-natively).
