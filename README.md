This SDK facilitates writing contracts in Rust.

## Contract Author Quickstart

Polished examples and tutorials are maintained outside this repository:
the [developer docs](https://docs.groundhog.dev/start/getting-started/) walk
through scaffolding a project with `groundhog new`, and
[groundhog-examples](https://github.com/groundhogchain/groundhog-examples)
holds worked contracts. (The `example-contracts/` directory here is an
internal test corpus for SDK development, not a set of recommended starting
points.)

Contract crates using the native test harness typically need:

```toml
[features]
# Required: the #[contract] macro gates guest entrypoint codegen on this.
entrypoints = []
dev = ["sdk/dev"]

[lib]
crate-type = ["rlib"]

[dependencies]
sdk = { path = "../sdk_rs/crates/sdk" }

[dev-dependencies]
my-contract = { path = ".", features = ["dev"] }
```

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

### Cloning

```shell
git clone ${REPO_URL}
```

Do **not** pass `--recurse-submodules`. This repository has one
[submodule](https://git-scm.com/book/en/v2/Git-Tools-Submodules),
`integration_test/contracts`, and `.gitmodules` points it at
`ssh://git@github.com/groundhogchain/integration_test_contracts.git` — a
private repository, so the recursive clone fails for anyone outside the
team.

It is internal-only. The one thing that needs its contents is the
integration block of `scripts/dev.sh ci` — `itc_build_rv32_all`,
`itc_test_all` and `itc_bottle_test_all` (`scripts/dev.sh:937-941`) — which
`SKIP_INTEGRATION=1` skips. (`dev.sh lint` also walks
`integration_test/contracts`, but an unpopulated directory yields no
packages, so it is a no-op there.) Everything else — the SDK's own tests,
the docs build, `example-contracts`, and the quality checks — runs without
the submodule.

Team members with access populate it the usual way:

```shell
git submodule update --init --recursive
```

## Requirements

### Rust toolchain

You can use [rustup](https://rustup.rs/) to get a Rust toolchain.

Then install the RV32 contract target:

```shell
rustup +nightly target add riscv32im-unknown-none-elf
```

Also, install the rust sources so the build process can
recompile them in a more compact form to reduce code size:

```shell
rustup component add rust-src --toolchain nightly
```

### Ccache

Install [ccache](https://ccache.dev/) with your package manager.

## Building

```shell
# Run the CI test to ensure everything builds; this also runs many tests
./scripts/ci.sh
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
