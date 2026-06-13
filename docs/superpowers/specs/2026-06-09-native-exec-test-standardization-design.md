# Native-exec test framework standardization

**Date:** 2026-06-09
**Status:** Approved design, pre-implementation
**Goal:** One canonical test pattern, maximal long-term cleanliness. Breaking
changes are acceptable (no deprecation cycle; semver bump).

## Background

Test code across sdk_rs uses six overlapping patterns spread over three layers:

1. **Fluent core** (`crates/native-exec`): `env.transact(|tx|
   tx.call(&addr).run(|| ...))` returning `TransactResult` with rich
   assertions. The newest layer; used by `crates/native-exec/tests/` and a few
   SDK tests (`rcq.rs`, `nnint_concurrent.rs`).
2. **Wrapper zoo** (`crates/sdk/src/mock/contract_test.rs`): eight public
   functions (`contract_run`, `contract_exec`, `contract_test`,
   `contract_test_raw`, `contract_test_handler`, `contract_handler_exec`,
   `contract_handler_exec_raw`, `client_test`) plus the `contract_test!`
   macro. These bypass the fluent core and drive the one-shot
   `TransactionHandler` via `env.prepare_test`. Used by ~40 example-contract
   tests and standard-tests.
3. **Ad-hoc helpers**: `example-contracts/swap/tests/common/` and
   `example-contracts/swap_driver/tests/common/` (~330 lines each) wrap the
   framework with `exec_tx`, which smuggles return values out of test closures
   via `Arc<Mutex<Option<T>>>`. The same hack appears in
   `standard-tests/erc20-native-test`. These exist because no official entry
   point lets a test closure produce a value.

The external `groundhog-examples` repo (registry dep on sdk) already
converged on the fluent pattern and is the model for the conventions below.

## The standard

### One execution pattern

```rust
let env = TestEnvironment::no_cost();
let token = erc20::register_mock_contract(&env);
let token_hash = *token.as_hash();
let alice = ContractAddress::named("alice");
let token_addr = deploy_address(&alice, &NONCE, &token_hash); // deterministic

let result = env.transact(|tx| {
    tx.call(&alice).run(|| {
        let addr = deploy_contract(&NONCE, &token_hash);
        let client = Erc20Client::new(&addr);
        client.initialize(1000);
        client.balance_of(&bob)            // closure returns a value
    })
});
result.assert_success();
let balance: Balance = result.assert_return_as();   // decoded from return buffer
result.assert_emitted::<Transfer>(&token_addr, |e| e.amount == 1000);
```

Multi-transaction / multi-block tests use the same chain repeatedly with
`env.advance_block()` between transactions. Nested calls use
`run_continuable` + `frame.child_call(&addr).run(|| ...)`, whose child
`FrameResult` supports the same decode methods. Manual commit/abort control
uses `env.transact_manual`.

There is deliberately **no** `env.call`-style one-line wrapper: a second
spelling of the same operation is the disease being treated. Going from one
transaction to several is adding lines, never changing shape.

### Return values travel through the return buffer

`FrameBuilder::run` becomes generic:

```rust
pub fn run<T: WriteXDR>(self, f: impl FnOnce() -> T + UnwindSafe) -> FrameResult
```

The closure's return value is XDR-encoded into the frame's return buffer at
frame close — exactly what macro-generated `#[contract]` entrypoints do with
their Rust return values. No live Rust object crosses the frame boundary, no
`Arc<Mutex>` plumbing, and every test round-trips its result through real
serialization.

`FrameResult`, `transact`, and `TransactResult` stay non-generic and
unchanged; existing call sites whose closures return `()` compile as-is.

Typed extraction joins the existing check-closure forms on both `FrameResult`
and `TransactResult` (delegated):

```rust
pub fn assert_return_as<R: ReadXDROwned>(&self) -> R   // asserts frame success, decodes return buffer
pub fn assert_abort_as<E: ReadXDROwned>(&self) -> E    // asserts abort, decodes abort buffer
```

Note: on abort there is no closure value by construction (the frame unwinds at
the abort point); `assert_abort_as` returns the decoded abort error, which is
the only value that exists in that state. `assert_fault()`, `assert_abort()`,
and all event assertions are unchanged.

**Implementation note (resolve in the plan):** `T = ()` must produce the same
empty-buffer frame close as today's no-return path (Rust cannot specialize on
`()`). If unit does not encode to the no-return outcome under the mock syscall
semantics, split into `run` (unit) and `run_returning<T>` instead of one
generic method. Also verify behavior when the closure both returns a non-unit
`T` and explicitly calls `raw_Return`: the expected rule is that an explicit
`raw_Return` takes precedence and the wrapper encodes the closure's value only
when no return was already set; if the syscall semantics make that
unimplementable, a double return must panic loudly rather than silently
overwrite. Either way, the chosen behavior gets a test.

### Sugar (exactly two entries)

Both are thin, visible delegations to the canonical chain, for tests with no
setup of their own:

- `contract_test!(name { ... })` — unit-style tests of SDK features (storage,
  nnints, hashing). Call syntax unchanged for existing users; reimplemented
  over `env.transact` (currently it routes through `contract_exec` /
  `prepare_test`).
- `client_test(register_fn, |client| ...)` — single-contract, client-driven
  tests. Syntax unchanged; reimplemented over `env.transact`.

### Conventions (ratified from groundhog-examples)

- Per test file: local `fn setup() -> (TestEnvironmentHandle, ...hashes)`.
- `TestEnvironment::no_cost()` unless storage costs are under test.
- **Mock registration uses the linkme auto-registry, never manual `__mock`
  listing.** Every `#[contract]` impl auto-registers a `ContractMockFragment`
  in `sdk::mock_registry::CONTRACT_MOCK_FRAGMENTS`; the generated
  `register_mock_contract(&env)` (from `declare_mock_contract!()`, backed by
  `register_mock_package`) assembles all impls in the crate — including
  multi-trait contracts — automatically. Manual
  `env.register_mock(|c| c.register(<T as Tr>::__mock)...)` is reserved for
  cross-crate composition or deliberately partial mocks.
- `ContractAddress::named("alice")` for caller/test addresses;
  `deploy_contract(&NONCE, &hash)` for deployments.

## Deletions (now, no deprecation)

From `crates/sdk/src/mock/contract_test.rs`:
- `contract_run`, `contract_exec`, `contract_test`, `contract_test_raw`,
  `contract_test_handler`, `contract_handler_exec`,
  `contract_handler_exec_raw` (the `client_test` function survives,
  reimplemented).

From `crates/native-exec`:
- `TestEnvironment::prepare_test`.
- `TestEnvironment::add_contract` as *public* API (becomes pub(crate) /
  internal; `register_mock` and the linkme registry subsume it).

From test code:
- `example-contracts/swap/tests/common/` and
  `example-contracts/swap_driver/tests/common/`: `exec_tx`, `Etx`, and all
  `Arc<Mutex>` plumbing deleted; each shrinks to a small `setup()`.
- `standard-tests/erc20-native-test`: `Arc<Mutex>` model extraction replaced
  by returning the model through the return buffer.

## Kept, with narrowed purpose

`TransactionHandler`, `RootInvokeResult`, `test_wasm`, and
`load_wasm_contract` remain **solely** for executing real wasm blobs through
the full `SignedTransaction` path (entrypoint dispatch, gas accounting —
`gas_consumed` is not available on the frame path). They are documented as the
wasm-execution harness, not a general test entry point. Native tests that
measure gas do so via `stats::gas_remaining()` inside the closure (as
bottle-test already does).

Semantic note: closure tests now run through `MockTransactionHandler` frames
rather than the real-`SignedTransaction` path the old wrappers used. That path
remains covered by `test_wasm` users and hogd's own tests.

## Migration scope (everything in-tree)

| Area | Change |
| --- | --- |
| `crates/native-exec` | `run` generic over `T: WriteXDR`; `assert_return_as` / `assert_abort_as`; `prepare_test` deleted; `add_contract` de-pub'd |
| `crates/sdk/src/mock/` | Delete 7 wrappers; reimplement `contract_test!` + `client_test` over `env.transact` |
| `crates/sdk/tests/` (~12 files) | Macro users unchanged; handler-based files (`events.rs`, etc.) move to `env.transact` |
| `example-contracts/` (~40 files) | `contract_exec(env, ...)` → `env.transact(...)`; swap / swap_driver `common/` dirs collapse; manual `__mock` lists → `register_mock_contract` |
| `standard-tests/` | ERC20 model-driven tests move to `env.transact`, model returned via return buffer, event checks via `TransactResult` |
| `logging-test/`, `crates/bottle-test/` | Move to `env.transact`; gas via `stats::gas_remaining()` in-closure |
| `integration_test/contracts/` (submodule) | Same edits; commits handled separately by Geoff |

groundhog-examples is out of scope for this change (registry-pinned); it
migrates on its next SDK upgrade, and already matches the conventions except
for `multi-impl`'s manual `__mock` listing and `cross-call`'s `client_test`
usage (which survives).

## Verification

- New coverage in `crates/native-exec/tests/fluent_api.rs`: value return
  through the buffer (round-trip), `()` behavior identical to legacy
  no-return, abort ⇒ `assert_abort_as` decodes, explicit-`raw_Return`
  interaction, child-frame return decoding.
- `cargo test` across the workspace (these are native tests; the wasm-only CI
  caveat applies to RV32 lowering, not to this).
- `cargo doc` builds clean (the deleted items are referenced from doc
  comments).
