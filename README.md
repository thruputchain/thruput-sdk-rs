This SDK facilitates writing contracts in Rust.

## Contract Author Quickstart

The canonical way to start a contract is the CLI scaffold. It writes the
manifest, `src/lib.rs`, the `#![no_main]` `src/main.rs` wrapper and a
`rust-toolchain.toml`, and `scripts/scaffold-smoke.sh` runs exactly this
path — `groundhog new`, then `groundhog build` — against this checkout on
every push to `main`:

```shell
groundhog new my-contract
cd my-contract
groundhog build          # compiles and lowers the rv32 artifact
```

Installing the CLI goes through the private Groundhog registry today, which
is what this repository's own scripts do (`scripts/ci.sh`,
`scripts/scaffold-smoke.sh`):

```shell
cargo install groundhog-cli --registry groundhog --locked
```

`cargo install` resolves configuration from the directory you run it in, so
this repository's `.cargo/config.toml` does not apply unless you happen to be
standing in the checkout. Set the index and token globally instead:

```shell
export CARGO_REGISTRIES_GROUNDHOG_INDEX="sparse+https://thruput.network/crates/api/v1/crates/"
export CARGO_REGISTRIES_GROUNDHOG_TOKEN=...
```

CI additionally installs the exact version pinned in
`scripts/groundhog-cli-version.sh`. There is no public install path yet —
distribution is tracked in
[groundhogchain/packaging#86](https://github.com/groundhogchain/packaging/issues/86).

Polished examples and tutorials are maintained outside this repository:
the [developer docs](https://docs.groundhog.dev/start/getting-started/) walk
through scaffolding a project with `groundhog new`, and
[groundhog-examples](https://github.com/groundhogchain/groundhog-examples)
holds worked contracts. (The `example-contracts/` directory here is an
internal test corpus for SDK development, not a set of recommended starting
points.)

### The contract crate

A contract crate is an ordinary Rust crate: an rlib holding the contract,
plus a thin rv32 binary that force-links it so the entrypoint records reach
the artifact. It may pull in as many dependencies as you like, but one
top-level build crate — the one with `src/main.rs` — becomes the deployed
artifact. Without that `[[bin]]` and its wrapper there is nothing to
deploy. [`crates/sdk/doc/lib.md`](crates/sdk/doc/lib.md) — the SDK's front
page, also readable through `./scripts/dev.sh open-docs` — explains each
piece. `groundhog new` writes this manifest:

```toml
[lib]
crate-type = ["rlib"]
doctest = false
test = false

# The rv32 artifact. The bin name must stay the snake_case package name:
# `groundhog build` looks for the ELF at `release/<snake_name>`. src/main.rs
# is a `#![no_std] #![no_main]` wrapper whose `extern crate my_contract;`
# force-links the rlib so the entrypoint records reach the binary.
[[bin]]
name = "my_contract"
path = "src/main.rs"
required-features = ["entrypoints"]
test = false
bench = false

# These names are contracts with the toolchain, not free choices:
# `#[contract]` keys guest entrypoint codegen on `entrypoints`, and
# `groundhog test` runs host tests with `--features dev`.
[features]
entrypoints = []
dev = ["sdk/dev"]

# `sdk` is declared per target: it needs `std` on the host and forbids it on
# the guest, and Cargo resolves features per platform.
[target.'cfg(target_arch = "riscv32")'.dependencies]
sdk = { version = "0.34.0", registry = "groundhog" }

[target.'cfg(not(target_arch = "riscv32"))'.dependencies]
sdk = { version = "0.34.0", registry = "groundhog", features = ["std"] }

# Deliberate self-dependency: turns `dev` on for `cargo test` builds only,
# keeping the library itself no_std for guest builds.
[dev-dependencies]
my-contract = { path = ".", features = ["dev"] }
```

The scaffold also wires a `groundhog-integration-test` feature and its
optional `groundhog-json-rpc-client`/`tokio` dependencies, for
`groundhog test --integration`; they are omitted above.

Native tests additionally link the hogd test-FFI library, downloaded on
first build by `crates/hogd-sys/build.rs`, so `cargo test` needs access to
that download as well as to the registry.

Tests run contracts natively against a `TestEnvironment` (an in-process
groundhog instance). The canonical pattern is the fluent transaction API:
each `env.transact` call builds one transaction, `tx.call(&addr).run(...)`
executes a closure as a contract frame at that address, and the returned
result is asserted on:

```rust,ignore
use sdk::*;

static NONCE: SHA256Hash = sdk::sha256_static!("my-test-nonce");

let env = TestEnvironment::no_cost();
let test_contract = my_contract::register_mock_contract(&env);
let contract_hash = *test_contract.as_hash();

env.transact(|tx| {
    tx.call(&ContractAddress::named("test")).run(move || {
        let addr = deploy_contract(&NONCE, &contract_hash);
        let client = MyContractClient::new(&addr);
        // exercise the generated client here
    })
})
.assert_success();
```

(`sdk::declare_mock_contract!();` once in `src/lib.rs` generates
`register_mock_contract`.)

For the common single-transaction case, `sdk::client_test` is thin sugar
over that pattern — it deploys the contract and hands the closure a client:

```rust,ignore
sdk::client_test(
    |env| my_contract::register_mock_contract(env),
    |client: &MyContractClient| {
        // exercise the generated client here
    },
);
```

Use the full `env.transact` form when a test needs multiple transactions,
asserts on aborts/faults, or decodes typed results via
`sdk::into_root_call_result`.

Generated clients default to `FooClient` for `#[contract] trait Foo` and for
`#[contract] impl Foo { ... }`. Use `client = CustomClient` only when you want
to override that default.

### Arithmetic on-chain

Deployed contracts are built with `overflow-checks = false` (see
`[profile.release]` in `example-contracts/.cargo/config.toml` and its
integration_test twin), so on-chain integer arithmetic — including
`sdk::U256` and `sdk::I256`, which deliberately mirror the primitives —
**silently wraps on overflow**. The native test harness compiles with
cargo's dev profile, where overflow checks are on, so the same expression
that panics (and fails the test) on the host wraps to a small number
on-chain. **A test passing under the harness proves nothing about overflow
behavior in deployment.**

For money math, be explicit instead of relying on operators:

- `checked_add` / `checked_sub` / `checked_mul` on the primitives and on
  `sdk::U256` / `sdk::I256`, combined with a typed throw on `None`;
- `sdk::mul_div_u128` / `sdk::checked_mul_div_u128` for `a * b / d` with
  a 256-bit intermediate, or `U256::mul_div` / `I256::mul_div` to stay at
  256 bits throughout, on an exact 512-bit intermediate (the panics of the
  `mul_div` family are unconditional, not `overflow-checks`-gated);
- storage-level guards: `NNInt::strict_inc` / `try_strict_inc` for a
  call-time non-negativity check, and `Balance::increase` / `decrease`,
  which fail on overflow and overdraft in every profile.

### 256-bit math utilities

`sdk::U256` and `sdk::I256` carry OpenZeppelin's `Math.sol` and
`SignedMath.sol` surfaces, so a port from Solidity keeps its vocabulary:
`mul_div` and `mul_shr` (full precision, 512-bit intermediate), `mul_mod` /
`add_mod` / `mod_exp` / `inv_mod`, `average`, `ceil_div`, `pow`, and
`log2` / `log10` / `log256`.

Names match Solidity's modulo casing, with these exceptions:

| Solidity | Here |
|---|---|
| `sqrt(a)` | `integer_sqrt` — the name the crate shipped with |
| `sqrt(a, Rounding)`, `log2(v, Rounding)`, … | `sqrt_rounding`, `log2_rounding`, `log10_rounding`, `log256_rounding` |
| `tryAdd` / `trySub` / `tryMul` / `tryDiv` / `tryMod` | `checked_add` and friends |
| `mul512`, `add512` | `widening_mul_u256`, `overflowing_add` — pairs, ordered `(low, high)` |
| `clz` | `leading_zeros` |
| `ternary` | `if` — it is a branchless select for the EVM's gas model |

Every fallible operation comes as a `checked_` form returning `Option` and a
panicking one; `U256::mul_div` aborts where `U256::checked_mul_div` reports.
There is no `U512`, and none is needed: Solidity has no type wider than
`uint256` either, and the only consumers of a 512-bit intermediate in
`Math.sol` — `mulDiv` and `mulShr` — are here whole.

A plain `git clone` is sufficient to build and test the SDK itself. The one
submodule, `integration_test/contracts`, is a private test corpus, and
`git submodule update --init` will fail without access to it. It is consumed
by `./scripts/dev.sh test-itc` and by the integration block of
`./scripts/ci.sh`; without the submodule, skip that block with
`SKIP_INTEGRATION=1 ./scripts/ci.sh`.

## Requirements

### Rust toolchain

Install [rustup](https://rustup.rs/). `rust-toolchain.toml` pins the nightly
toolchain this repository builds with and lists the `riscv32im-unknown-none-elf`
target and the `rust-src` component, so rustup installs them automatically on
the first `cargo` invocation. No manual `rustup target add` step is needed.

### Optional: ccache

[ccache](https://ccache.dev/) is only used when building the hogd test
executor from source (`HOGD_FFI_BUILD=1`). The default prebuilt download path
does not need it.

## Building

```shell
# Run the CI test to ensure everything builds; this also runs many tests
./scripts/ci.sh

# Without access to the integration_test/contracts submodule
SKIP_INTEGRATION=1 ./scripts/ci.sh
```

## Documentation

To generate and open the SDK documentation:

```shell
./scripts/dev.sh open-docs
```

## Development

### Testing

The script `scripts/dev.sh` offers various commands useful in development,
e.g.:

```shell
alias dev='./scripts/dev.sh'

dev build-examples-rv32

# Expand macros to view internals of contract wrappers, etc.
dev expand example-contracts/erc20
```

### Benchmarking 256-bit arithmetic

`dev bigint-bench` reports rv32 codesize and runtime gas per operation for each
256-bit integer implementation under test — today `sdk::U256` against Parity's
`uint` — so a change to `crates/u256` can be diffed against the implementation it
replaced. Where `uint` has no equivalent entry point (its full-precision
`mulDiv`), the backend supplies the one a contract author on that backend would
have to write, and `cargo test -p bigint-bench-contract` asserts the two agree
before any number is believed. It builds the benchmark contract once per (implementation, operation)
pair, which is minutes of work, so it is not part of `ci`:

```shell
dev bigint-bench                 # full sweep, both metrics
dev bigint-bench -o 'div sqrt'   # one or two operations, while iterating
dev bigint-bench-test            # host check that the backends agree
```

Design and current numbers:
`docs/superpowers/specs/2026-08-12-bigint-bench-design.md`.

### Diagnostics

When running tests:
- Set `SDK_TEST_ENABLE_SYSCALL_LOGGING=1` to see a log of syscalls made
- Set `TEST_FFI_TRACE=1` to see interactions with the test executor. When
  reporting a bug, it's helpful to include a trace captured with
  `TEST_FFI_TRACE=1`.
