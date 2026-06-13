# Native-exec Test Standardization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Converge all test code on one execution pattern — `env.transact(|tx| tx.call(&addr).run(|| ...))` with values crossing the frame boundary via the XDR return buffer — and delete the legacy wrapper layer.

**Architecture:** Three phases: (1) add the new capabilities to `crates/native-exec` (generic `run`, typed buffer extraction), (2) rebuild the two surviving sugar entries (`contract_test!`, `client_test`) and one result adapter in `crates/sdk`, then migrate every in-tree call site, (3) delete the old API. Old and new coexist until the final deletion task so the workspace stays green after every task.

**Tech Stack:** Rust; `xdrs` (`WriteXDR`/`ReadXDROwned`, `cxs::encode`/`cxs::decode`); `groundhog_xdr::syscalls::raw_Return`; linkme mock registry.

**Spec:** `docs/superpowers/specs/2026-06-09-native-exec-test-standardization-design.md`

**Known deviation from spec (flagged for review):** the spec says delete `exec_tx`/`Etx` in swap/swap_driver entirely. This plan instead keeps `exec_tx` as a ~8-line domain fixture reimplemented on the new API (no `Arc<Mutex>`) and keeps `Etx`'s domain helpers, because they are test-domain abstractions (user balances, stats snapshots), not framework workarounds — deleting them would force rewriting 4 test files into repetitive inline plumbing. The `Arc<Mutex>` machinery (`_exec_tx`, the smuggling cell) is fully deleted. If full deletion is still wanted, say so before Task 6 executes.

**Commit rules:**
- NEVER `git add` anything under `docs/superpowers/` or `integration_test/contracts/` (submodule; Geoff commits it separately).
- One commit per task, message format given in each task.

---

## Background for the implementing engineer

Key files and existing API (read these before starting):
- `crates/native-exec/src/frame_builder.rs` — `FrameBuilder::run(self, f: impl FnOnce() + UnwindSafe) -> FrameResult` executes `f` inside a mock invocation frame via `frame.execute(|_| { f(); FrameAction::Close })`.
- `crates/native-exec/src/frame_result.rs` — `FrameResult` wraps `InvocationFrameOutput` (Return/Abort/Fault) + events; has `assert_return_with`/`assert_abort_with` (check-closure style), `return_buffer()`, `abort_buffer()`.
- `crates/native-exec/src/transact_result.rs` — `TransactResult` = `FrameResult` + commit `ExecResult`; delegates most assertions to the frame.
- `crates/native-exec/src/test_environment.rs` — `transact`, `transact_manual`, `register_mock`, `prepare_test` (to be deleted), `add_contract` (to be doc-hidden).
- `crates/sdk/src/mock/contract_test.rs` — the legacy wrapper zoo (to be deleted) plus `contract_test!`, `client_test`, `test_wasm` (kept).
- Inside a frame closure, contract code can call `groundhog_xdr::syscalls::raw_Return(&bytes, /*revert=*/false, &[])` to set the frame's return buffer; closing a frame without calling it produces `Return` with an empty buffer. `revert=true` produces an Abort.
- XDR codec: `xdrs::cxs::encode(&v) -> WriteResult<Vec<u8>>` for `v: WriteXDR`; `xdrs::cxs::decode::<T>(buf)` for `T: ReadXDROwned`. `()` encodes to zero bytes. Tuples, fixed arrays, `Vec<T>`, `Option<T>`, `String` all have impls.

---

### Task 1: `FrameBuilder::run` returns values through the return buffer

**Files:**
- Modify: `crates/native-exec/src/frame_builder.rs:40-47`
- Test: `crates/native-exec/tests/fluent_api.rs`

- [ ] **Step 1: Write failing tests** — append to `crates/native-exec/tests/fluent_api.rs`:

```rust
//-------------------------------------------------------------------------
// Generic run(): closure return values travel through the return buffer.

#[test]
fn test_run_encodes_closure_return_value() {
    let env = groundhog_native_exec::test_environment::TestEnvironment::new(None);
    let result = env.transact(|tx| tx.call(&TEST_ADDR).run(|| 42u32));
    result.assert_success();
    let decoded: u32 = xdrs::cxs::decode(result.frame().return_buffer()).unwrap();
    assert_eq!(decoded, 42);
}

#[test]
fn test_run_unit_closure_leaves_buffer_empty() {
    let env = groundhog_native_exec::test_environment::TestEnvironment::new(None);
    let result = env.transact(|tx| tx.call(&TEST_ADDR).run(|| ()));
    result.assert_success();
    assert!(result.frame().return_buffer().is_empty());
}

#[test]
fn test_run_unit_closure_with_explicit_raw_return_is_preserved() {
    // Legacy style: unit closure sets the buffer itself. The wrapper must
    // not emit a second return (unit encodes to zero bytes -> skipped).
    let env = groundhog_native_exec::test_environment::TestEnvironment::new(None);
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            groundhog_xdr::syscalls::raw_Return(&[7u8, 8, 9], false, &[]);
        })
    });
    result.assert_success();
    assert_eq!(result.frame().return_buffer(), &[7, 8, 9]);
}

#[test]
fn test_run_tuple_return_roundtrip() {
    let env = groundhog_native_exec::test_environment::TestEnvironment::new(None);
    let result =
        env.transact(|tx| tx.call(&TEST_ADDR).run(|| (1u64, String::from("hi"))));
    result.assert_success();
    let decoded: (u64, String) =
        xdrs::cxs::decode(result.frame().return_buffer()).unwrap();
    assert_eq!(decoded, (1, String::from("hi")));
}

#[test]
fn test_run_panic_produces_fault_not_value() {
    let env = groundhog_native_exec::test_environment::TestEnvironment::new(None);
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| -> u32 { panic!("boom") })
    });
    result.assert_fault();
}
```

If `fluent_api.rs` lacks an `xdrs` dev-dependency path, it is already a dependency of native-exec (`xdrs::txs` is used in src); add `xdrs.workspace = true` to `[dev-dependencies]` in `crates/native-exec/Cargo.toml` only if the test fails to resolve it.

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test -p groundhog-native-exec --test fluent_api test_run_ -- --nocapture`
Expected: compile error (closures returning non-`()` don't match `FnOnce()`).

- [ ] **Step 3: Implement generic `run`** — replace `FrameBuilder::run` in `crates/native-exec/src/frame_builder.rs`:

```rust
    /// Execute the frame with the given closure, then close it.
    ///
    /// The closure's return value is XDR-encoded into the frame's return
    /// buffer (the same convention generated `#[contract]` entrypoints use),
    /// and can be recovered from the result via `assert_return_as`.
    /// A `()` return encodes to zero bytes and leaves the buffer untouched,
    /// so closures that call `raw_Return` themselves keep working.
    ///
    /// If you need to open child frames from within the closure, use
    /// `run_continuable` instead, which provides a `FrameHandle` with
    /// `child_call()`.
    pub fn run<T: xdrs::WriteXDR>(
        self,
        f: impl FnOnce() -> T + UnwindSafe,
    ) -> FrameResult {
        let frame = self.config.build();
        frame.execute(|_| {
            let value = f();
            let encoded = xdrs::cxs::encode(&value)
                .expect("run(): failed to XDR-encode closure return value");
            if !encoded.is_empty() {
                groundhog_xdr::syscalls::raw_Return(&encoded, false, &[]);
            }
            FrameAction::Close
        });
        frame.result()
    }
```

If `raw_Return`'s third parameter is not `&[u8]`-compatible, mirror the exact call form already used at `crates/native-exec/tests/fluent_api.rs:466`.

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test -p groundhog-native-exec`
Expected: all PASS, including pre-existing tests (existing `run(|| ...)` call sites return `()` and are unaffected).

- [ ] **Step 5: Pin the unsupported-mix behavior** (spec requirement). Write this probe test, run it, observe what the syscall layer does when `raw_Return` is called twice (explicit call + wrapper encoding a non-unit value), then set the test's assertion to the observed behavior and document it in `run`'s doc comment ("mixing an explicit `raw_Return` with a non-unit closure return is unsupported; the observed behavior is X"):

```rust
#[test]
fn test_run_nonunit_return_after_explicit_raw_return_pins_behavior() {
    // Unsupported mix: closure sets the buffer explicitly AND returns a
    // non-unit value. This test pins whatever the syscall layer does
    // (overwrite / fault) so a behavior change is caught.
    let env = groundhog_native_exec::test_environment::TestEnvironment::new(None);
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            groundhog_xdr::syscalls::raw_Return(&[1u8], false, &[]);
            42u32
        })
    });
    // Set the assertion to the OBSERVED outcome when first run:
    // e.g. `result.assert_success();` + buffer check, or `result.assert_fault();`
    todo!("replace with observed behavior before committing");
}
```

Run: `cargo test -p groundhog-native-exec --test fluent_api pins_behavior -- --nocapture`, observe, replace the `todo!` with assertions matching the output, re-run to green. The committed test must contain no `todo!`.

- [ ] **Step 6: Commit**

```bash
git add crates/native-exec
git commit -m "native-exec: run() closures return values through the XDR return buffer"
```

---

### Task 2: `assert_return_as` / `assert_abort_as` typed extraction

**Files:**
- Modify: `crates/native-exec/src/frame_result.rs` (after `assert_abort_with`, ~line 160)
- Modify: `crates/native-exec/src/transact_result.rs` (after `assert_abort_with`, ~line 66)
- Test: `crates/native-exec/tests/fluent_api.rs`

- [ ] **Step 1: Write failing tests** — append to `fluent_api.rs`:

```rust
#[test]
fn test_assert_return_as_decodes_value() {
    let env = groundhog_native_exec::test_environment::TestEnvironment::new(None);
    let result = env.transact(|tx| tx.call(&TEST_ADDR).run(|| 1234u64));
    result.assert_success();
    let v: u64 = result.assert_return_as();
    assert_eq!(v, 1234);
}

#[test]
#[should_panic(expected = "failed to decode return buffer")]
fn test_assert_return_as_panics_on_garbage() {
    let env = groundhog_native_exec::test_environment::TestEnvironment::new(None);
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            groundhog_xdr::syscalls::raw_Return(&[0xFF], false, &[]);
        })
    });
    let _: u64 = result.assert_return_as();
}

#[test]
fn test_assert_abort_as_decodes_error() {
    let env = groundhog_native_exec::test_environment::TestEnvironment::new(None);
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            let encoded = xdrs::cxs::encode(&77u32).unwrap();
            groundhog_xdr::syscalls::raw_Return(&encoded, true, &[]);
        })
    });
    result.assert_abort();
    let e: u32 = result.assert_abort_as();
    assert_eq!(e, 77);
}
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p groundhog-native-exec --test fluent_api assert_return_as`
Expected: compile error (method does not exist).

- [ ] **Step 3: Implement** — in `frame_result.rs`, alongside the existing `_with` forms:

```rust
    /// Assert the frame succeeded and decode the return buffer, returning
    /// the decoded value. The returning twin of `assert_return_with`.
    #[track_caller]
    pub fn assert_return_as<R>(&self) -> R
    where
        R: xdrs::ReadXDROwned,
    {
        self.assert_success();
        xdrs::cxs::decode(self.return_buffer()).unwrap_or_else(|e| {
            panic!("failed to decode return buffer: {}", e)
        })
    }

    /// Assert the frame aborted and decode the abort buffer, returning
    /// the decoded error. The returning twin of `assert_abort_with`.
    #[track_caller]
    pub fn assert_abort_as<E>(&self) -> E
    where
        E: xdrs::ReadXDROwned,
    {
        self.assert_abort();
        xdrs::cxs::decode(self.abort_buffer()).unwrap_or_else(|e| {
            panic!("failed to decode abort buffer: {}", e)
        })
    }
```

In `transact_result.rs`, delegate:

```rust
    /// Assert the root frame returned successfully and decode its return
    /// buffer, returning the decoded value. Frame-only check, like
    /// `assert_return_with`.
    #[track_caller]
    pub fn assert_return_as<R>(&self) -> R
    where
        R: xdrs::ReadXDROwned,
    {
        self.frame.assert_return_as()
    }

    #[track_caller]
    pub fn assert_abort_as<E>(&self) -> E
    where
        E: xdrs::ReadXDROwned,
    {
        self.frame.assert_abort_as()
    }
```

- [ ] **Step 4: Run, verify pass**

Run: `cargo test -p groundhog-native-exec`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/native-exec
git commit -m "native-exec: add assert_return_as/assert_abort_as typed buffer extraction"
```

---

### Task 3: Rebuild sdk sugar on the fluent core; add the RawCallError adapter

**Files:**
- Modify: `crates/sdk/src/mock/contract_test.rs`

Do NOT delete the legacy functions yet (Task 9). This task changes only `contract_test!`, `client_test`, and adds `into_root_call_result`.

- [ ] **Step 1: Replace the `contract_test!` macro** (currently routes through `contract_exec`):

```rust
/// Run some code in the test environment, as a single auto-committed
/// transaction at a deterministic per-test address.
///
#[macro_export]
macro_rules! contract_test {
    ($name:ident $test_body:tt) => {
        #[test]
        fn $name() {
            let env = $crate::TestEnvironment::no_cost();
            env.transact(|tx| {
                let addr = <$crate::ContractAddress as $crate::Named>::named(
                    stringify!($name),
                );
                tx.call(&addr).run(|| $test_body)
            })
            .assert_success();
        }
    };
}
```

- [ ] **Step 2: Replace `client_test`** (keep the `TestRegisterFn` trait as is):

```rust
pub fn client_test<R, F, C>(register: R, run_test: F)
where
    R: TestRegisterFn,
    F: FnOnce(&C) + UnwindSafe,
    C: crate::call::ContractClient,
{
    static CLIENT_TEST_NONCE: SHA256Hash =
        SHA256Hash::new(hash_to_array!("CLIENT_TEST_NONCE"));

    let env = TestEnvironment::no_cost();
    let test_contract = register(&env);
    let contract_hash = *test_contract.as_hash();

    env.transact(|tx| {
        let caller = <ContractAddress as Named>::named("client_test");
        tx.call(&caller).run(move || {
            // Deploy the contract under test and hand the caller a client.
            let addr = deploy_contract(&CLIENT_TEST_NONCE, &contract_hash);
            let client = C::new(&addr);
            run_test(&client);
        })
    })
    .assert_success();
}
```

If `Named` is not already imported in this file, add `use groundhog_native_exec::test_helpers::Named;` (it is re-exported from `sdk::mock`).

- [ ] **Step 3: Add the adapter** — append to `contract_test.rs` (this preserves the
`Result<T, RawCallError>` assertion style used by exception-decoding tests):

```rust
/// Decode a `TransactResult`'s root-frame outcome into the same
/// `Result<T, RawCallError>` shape that contract clients produce.
/// Use this when a test asserts on typed exceptions/selectors rather
/// than raw buffers.
pub fn into_root_call_result<T>(
    result: &TransactResult,
) -> Result<T, RawCallError>
where
    T: ReadXDROwned + ReceiveAuthHandles,
{
    let contract_result: ContractResult = match result.frame().output() {
        Return { return_buffer } => Ok((
            ReturnBuf::from_slice(return_buffer.as_slice()).unwrap(),
            AuthHandleBuf::new(),
        )),
        Abort { abort_buffer } => Err(ContractError::Abort(
            AbortBuf::from_slice(abort_buffer.as_slice()).unwrap(),
        )),
        Fault => Err(ContractError::Fault(FaultString::empty())),
        other => {
            panic!("unsupported InvocationFrameOutput variant: {:?}", other)
        }
    };
    crate::call::into_raw_call_result(contract_result)
}
```

(The `Return`/`Abort`/`Fault` variants are already glob-imported at the top of this file via `use crate::types::transaction::InvocationFrameOutput::*;`.)

- [ ] **Step 4: Verify** — the macro change affects ~6 sdk test files that use `contract_test!`:

Run: `cargo test -p sdk --tests`
Expected: PASS (storage.rs, delete.rs, bytestrings.rs, hasher.rs, internal_nnints.rs, nnints.rs compile and pass with the new macro; files still using `contract_exec` directly are untouched and still pass).

Run: `cargo test -p hasher -p stateless` (example-contracts `client_test` users)
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sdk
git commit -m "sdk: rebuild contract_test!/client_test on env.transact; add into_root_call_result"
```

---

### Task 4: Migrate crates/sdk tests off the legacy wrappers

**Files:**
- Modify: `crates/sdk/tests/nnints.rs`, `crates/sdk/tests/internal_nnints.rs`, `crates/sdk/tests/aborts.rs`, `crates/sdk/tests/events.rs`, `crates/sdk/tests/root_error_codes.rs`

The recipe for every direct `contract_exec(env, || BODY)` call whose result feeds
`assert_syscall_exception` / `RawCallError` matching:

```rust
// OLD
let res = contract_exec(TestEnvironment::no_cost(), || { BODY });
assert_syscall_exception(&res, CALLNO, ERRNO);

// NEW
let env = TestEnvironment::no_cost();
let result = env.transact(|tx| {
    tx.call(&<ContractAddress as Named>::named("nnints")).run(|| { BODY })
});
let res: Result<(), sdk::call::RawCallError> = into_root_call_result(&result);
assert_syscall_exception(&res, CALLNO, ERRNO);
```

For success-only checks (`if !res.is_ok() { panic!(...) }`), use `result.assert_success();` instead of the adapter.

- [ ] **Step 1: Migrate `nnints.rs` and `internal_nnints.rs`** using the recipe above (≈7 and ≈3 direct call sites respectively; the `contract_test!`-macro tests in these files need no edits). `assert_syscall_exception` lives in `crates/sdk/tests/common/mod.rs` and keeps its signature.

- [ ] **Step 2: Rewrite `aborts.rs`** — full new content:

```rust
use sdk::call::RawCallError;
use sdk::*;

fn run_at_test_addr(
    f: impl FnOnce() + std::panic::UnwindSafe,
) -> TransactResult {
    let env = TestEnvironment::no_cost();
    env.transact(|tx| {
        tx.call(&<ContractAddress as Named>::named("aborts")).run(f)
    })
}

#[test]
fn test_panic() {
    let result = run_at_test_addr(|| {
        panic!("Just panic");
    });
    result.assert_fault();
    let res: Result<(), RawCallError> = into_root_call_result(&result);
    assert!(
        matches!(res, Err(RawCallError::Fault(_))),
        "Unexpected invocation result: {:?}",
        res
    );
}

#[test]
fn test_exception_raw() {
    let result = run_at_test_addr(|| {
        contract_abort(&[1u8, 2, 3]);
    });
    result.assert_abort();
    assert_eq!(result.frame().abort_buffer(), &[1u8, 2, 3]);
}

#[test]
fn test_unparseable_return() {
    // The contract "returns successfully" but with garbage bytes that
    // can't be XDR-decoded as the expected return type.
    let result = run_at_test_addr(|| {
        sdk::contract_return_buf(&[0xFF]);
    });
    let res: Result<u64, RawCallError> = into_root_call_result(&result);
    assert!(
        matches!(res, Err(RawCallError::UnparseableReturn(_))),
        "Expected UnparseableReturn, got: {:?}",
        res
    );
}

#[test]
fn test_unparseable_abort() {
    // The contract aborts with garbage bytes that can't be parsed
    // as a RawException (needs at least a u64 selector).
    let result = run_at_test_addr(|| {
        contract_abort(&[0xFF]);
    });
    let res: Result<(), RawCallError> = into_root_call_result(&result);
    assert!(
        matches!(res, Err(RawCallError::UnparseableAbort(_))),
        "Expected UnparseableAbort, got: {:?}",
        res
    );
}
```

- [ ] **Step 3: Migrate `events.rs`** — three changes:

(a) Replace the `single_event_test!` macro body (file already defines `TEST_ADDR`):

```rust
macro_rules! single_event_test {
    {
        fn $test_name:ident() $body:tt
        fn event_check($event:ident) $body_event_check:tt
    } =>
    {
        #[test]
        fn $test_name() {
            let env = TestEnvironment::no_cost();
            let result =
                env.transact(|tx| tx.call(&TEST_ADDR).run(|| $body));
            result.assert_success();
            result.assert_event_count(1);

            let event_check = |$event: &TxEmittedEvent| $body_event_check;
            event_check(&result.events()[0]);
        }
    };
}
```

(b) Change `assert_encoding_event_exception` to take a `TransactResult` and use the adapter; update its two callers (~lines 327, 337):

```rust
fn assert_encoding_event_exception(result: TransactResult) {
    let res: Result<(), RawCallError> = into_root_call_result(&result);
    match res {
        // ... keep the existing match-arm bodies unchanged ...
    }
}

// callers become:
    assert_encoding_event_exception(env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            log_event(&ManualTooManyTopicsEvent { value: 7 });
        })
    }));
```

(c) Replace the `contract_test_handler` block in `test_field_order_does_not_affect_serialization` (~line 383):

```rust
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            // ... the three log_event calls, unchanged ...
        })
    });
    result.assert_success();
    let events = result.events();
    assert_eq!(events.len(), 3);
    // ... rest of the assertions unchanged, using `events` ...
```

- [ ] **Step 4: Rewrite `root_error_codes.rs`** — full new content:

```rust
use sdk::{ContractAddress, Named, TestEnvironment};

#[test]
fn test_root_frame_revert() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("root_error_codes")).run(|| {
            sdk::sys::raw_Return(&[], true, &[]);
        })
    });

    result.assert_abort();
    assert_eq!(
        *result.exec_result(),
        sdk::ExecResult::ExecResultTxRootFrameReverted
    );
}
```

- [ ] **Step 5: Verify**

Run: `cargo test -p sdk --tests`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/sdk
git commit -m "sdk tests: migrate to env.transact + into_root_call_result"
```

---

### Task 5: Migrate example-contracts off `contract_exec`/`contract_test`

**Files (15):** `example-contracts/{multi_throws,multi_imported_traits,named_interface_caller,exceptions,imported_error_caller,caller,versioned_caller,bare_impl_caller}/tests/native.rs`, `example-contracts/{exceptions,caller}/tests/wasm.rs`, `example-contracts/recursive_type/tests/test.rs`, `example-contracts/signer_caller/tests/forwarding.rs`, `example-contracts/signer_minter/tests/lifecycle.rs`, `example-contracts/erc1271_caller/tests/lifecycle.rs`, `example-contracts/counter/tests/test.rs`

The mechanical recipe (applies to every call site; wasm-loading tests differ only in setup, which is unchanged):

```rust
// OLD
contract_exec(env, move || { BODY })
    .unwrap_or_else(|e| panic!("Test failed: {e:?}"));

// NEW
env.transact(|tx| {
    tx.call(&ContractAddress::named("test")).run(move || { BODY })
})
.assert_success();
```

```rust
// OLD (error-expected variant)
let result: Result<(), sdk::call::RawCallError> =
    sdk::contract_test(env, move || { BODY });
assert!(result.is_err(), ...);

// NEW — if the test only needs "it aborted":
let result = env.transact(|tx| {
    tx.call(&ContractAddress::named("test")).run(move || { BODY })
});
result.assert_abort();

// NEW — if the test matches specific RawCallError variants/selectors:
let result = env.transact(|tx| {
    tx.call(&ContractAddress::named("test")).run(move || { BODY })
});
let res: Result<(), sdk::call::RawCallError> = into_root_call_result(&result);
// ...existing match logic unchanged...
```

Worked example — `example-contracts/counter/tests/test.rs`, first test becomes:

```rust
#[test]
fn test_counter_basic() {
    let (env, contract_hash) = setup();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&COUNTER_NONCE, &contract_hash);
            let client = CounterClient::new(&addr);

            client.initialize(10);
            assert_eq!(client.get(), 10);
            client.increment(5);
            assert_eq!(client.get(), 15);
            client.decrement(3);
            assert_eq!(client.get(), 12);
        })
    })
    .assert_success();
}
```

(`setup()` functions are already the standard shape and stay unchanged. Add `use sdk::Named;` where `sdk::*` glob doesn't already provide it — it does, via the mock re-exports.)

- [ ] **Step 1: Apply the recipe to all 15 files.** Per-file note: `caller/tests/wasm.rs` and `exceptions/tests/wasm.rs` load wasm contracts with `load_wasm_contract` + `env.setup_create_contract` first; only the `contract_exec(env, move || ...)` invocation changes, the wasm setup stays.

- [ ] **Step 2: Verify**

Run: `cargo test -p counter -p caller -p exceptions -p multi_throws -p multi_imported_traits -p named_interface_caller -p recursive_type -p signer_caller -p imported_error_caller -p signer_minter -p versioned_caller -p erc1271_caller -p bare_impl_caller`
(Adjust `-p` names to the actual crate names from each `Cargo.toml` if they differ; wasm tests additionally need the prebuilt wasm artifacts — if `../../target/optimized/*.wasm` is missing locally, build it the same way CI does or note the skip in the task report.)
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add example-contracts
git commit -m "example-contracts: migrate tests to env.transact pattern"
```

---

### Task 6: swap / swap_driver — delete the Arc<Mutex> machinery

**Files:**
- Modify: `example-contracts/swap_driver/tests/common/util.rs`, `example-contracts/swap/tests/common/util.rs`
- Modify (only if compile requires bound changes): `example-contracts/{swap,swap_driver}/tests/common/etx.rs`

- [ ] **Step 1: Rewrite `swap_driver/tests/common/util.rs`** — full new content:

```rust
// General debugging utilities

use sdk::*;
use std::panic::UnwindSafe;

pub type Result<T, E = TransactResult> = core::result::Result<T, E>;

#[track_caller]
pub fn transact_assert_success<F, T>(f: F) -> T
where
    F: Fn() -> Result<T>,
{
    // (Protect track_caller by avoiding a closure)
    match f() {
        Ok(v) => v,
        Err(r) => {
            r.assert_success();
            unreachable!();
        }
    }
}

/// Run `f` as a single auto-committed transaction at `addr`. On success the
/// closure's return value is decoded from the frame's return buffer; on
/// failure the full `TransactResult` is returned for inspection.
pub fn exec_tx<F, T>(
    env: &TestEnvironmentHandle,
    addr: &ContractAddress,
    f: F,
) -> Result<T, TransactResult>
where
    F: FnOnce() -> T + UnwindSafe,
    T: xdrs::WriteXDR + xdrs::ReadXDROwned,
{
    let r = env.transact(|tx| tx.call(addr).run(f));
    if r.is_success() {
        Ok(r.assert_return_as())
    } else {
        Err(r)
    }
}
```

- [ ] **Step 2: Rewrite `swap/tests/common/util.rs`** — same as Step 1 plus the `exec_tx2` event-handler variant that file also defines (keep the `use sdk::types::event::TxEmittedEvent;` import):

```rust
pub fn exec_tx2<F, T, E>(
    env: &TestEnvironmentHandle,
    addr: &ContractAddress,
    f: F,
    event_handler: E,
) -> Result<T, TransactResult>
where
    F: FnOnce() -> T + UnwindSafe,
    T: xdrs::WriteXDR + xdrs::ReadXDROwned,
    E: Fn(&TxEmittedEvent),
{
    let r = env.transact(|tx| tx.call(addr).run(f));
    if r.is_success() {
        for e in r.events() {
            event_handler(e);
        }
        Ok(r.assert_return_as())
    } else {
        Err(r)
    }
}
```

Delete `_exec_tx` and every `Arc`/`Mutex` use from both files.

- [ ] **Step 3: Fix `etx.rs` bounds in both crates** — `Etx::exec_tx`/`exec_user_tx` currently require `Fn() -> T + RefUnwindSafe`; change to match the new `exec_tx`:

```rust
    pub fn exec_user_tx<F, T>(&self, user: User, f: F) -> Result<T, TransactResult>
    where
        F: FnOnce(&User) -> T + std::panic::UnwindSafe,
        T: xdrs::WriteXDR + xdrs::ReadXDROwned,
    {
        exec_tx(&self.env, user.addr, move || f(&user))
    }
```

(and analogously for `Etx::exec_tx`). The other `Etx` helpers and `setup.rs` compile unchanged — their return types are tuples/arrays of XDR-able types. If a closure capture fails `UnwindSafe`, wrap the call site value in `core::panic::AssertUnwindSafe` only as a last resort and note it in the task report.

- [ ] **Step 4: Verify**

Run: `cargo test -p swap -p swap_driver` (adjust to actual crate names)
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add example-contracts/swap example-contracts/swap_driver
git commit -m "swap tests: replace Arc<Mutex> value smuggling with return-buffer exec_tx"
```

---

### Task 7: standard-tests + bottle-test + erc20 event derives

**Files:**
- Modify: `standard-interfaces/groundhog-erc20/src/events.rs`
- Modify: `standard-tests/erc20-test/src/model.rs`
- Modify: `standard-tests/erc20-native-test/src/lib.rs`
- Modify: `crates/bottle-test/src/main.rs`

- [ ] **Step 1: Add XDR derives** in `standard-interfaces/groundhog-erc20/src/events.rs` to `StdERC20Event` (line ~245) and to the owned event structs it wraps (the `stdevt::TransferEvent` / `stdevt::ApprovalEvent` definitions in the same file/module):

```rust
#[derive(Clone, Debug, PartialEq, Eq, xdrs::WriteXDR, xdrs::ReadXDR)]
pub enum StdERC20Event {
    Transfer(stdevt::TransferEvent),
    Approval(stdevt::ApprovalEvent),
}
```

Add `xdrs` to that crate's `Cargo.toml` dependencies if not present (`xdrs = { workspace = true }`). Note: this crate is published at `=0.1.2`; the source change rides along until the next version bump (handled by the release process, not this plan).

- [ ] **Step 2: Refactor `Model::check_events`** in `standard-tests/erc20-test/src/model.rs` into a free function so it can run without the (non-XDR-able) `Model`:

```rust
/// Compare an expected event log (from the model) against the events a
/// transaction actually emitted.
pub fn check_events(expected: &[StdERC20Event], events: &[TxEmittedEvent]) {
    let mut events = events.iter();
    for (i, model_event) in expected.iter().enumerate() {
        // ... existing body of Model::check_events, with `self.events`
        //     replaced by `expected` ...
    }
    // ... existing trailing-events panic unchanged ...
}

impl Model {
    pub fn check_events(&self, events: &[TxEmittedEvent]) {
        check_events(&self.events, events);
    }
}
```

- [ ] **Step 3: Rewrite `standard-tests/erc20-native-test/src/lib.rs`** — full new content:

```rust
pub use erc20_test::*;

use sdk::{ContractAddress, Named, SHA256Hash, TestEnvironmentHandle};

pub fn test_erc20_contract(
    env: TestEnvironmentHandle,
    test_contract_hash: &SHA256Hash,
) {
    let test_contract_hash = *test_contract_hash;

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("erc20_test")).run(move || {
            let deployed_addr = erc20_test_setup(&test_contract_hash);
            let model = erc20_test(&deployed_addr);
            // The model itself isn't XDR-able; its expected event log is,
            // and it's all the post-commit check needs.
            model.events
        })
    });
    result.assert_success();

    // Print out the transaction's event log
    {
        use groundhog_erc20_if::*;
        use sdk::EmittedEvent;

        for (i, event) in result.events().iter().enumerate() {
            let event: EmittedEvent = event.into();
            match ERC20Event::try_from(event) {
                Ok(event) => eprintln!("\tevent {:?}: {}", i, event),
                Err(_) => eprintln!("\tevent {:?}: <non-ERC20 event>", i),
            }
        }
    }

    // Check that the event log matches the model
    let expected: Vec<groundhog_erc20_if::StdERC20Event> =
        result.assert_return_as();
    check_events(&expected, result.events());
}
```

(`Vec<StdERC20Event>` encodes via xdrs's `Vec<T>` impls; `erc20-bottle-test` is contract code, not harness code, and needs no changes.)

- [ ] **Step 4: Rewrite the harness section of `crates/bottle-test/src/main.rs`** — replace everything from `let mut handler = contract_test_handler(...)` to the end of `bottle_test` with:

```rust
    let result = env.transact(|tx| {
        // OWNER is the address the legacy harness invoked at; keep it so
        // deploy_address(&OWNER, ...) computed above stays consistent.
        tx.call(&OWNER).run(move || {
            let tester_addr = deploy_contract(&TESTER_NONCE, &tester_hash);

            let tester = BottleTestClient::new(&tester_addr);
            tester.setup(&target_hash);

            let gas_start = stats::gas_remaining();
            tester.run();
            let gas_end = stats::gas_remaining();

            let gas_consumed = gas_start
                .checked_sub(gas_end)
                .unwrap_or_else(|| panic!("Negative gas consumed!"));

            eprintln!("Bottle test succeeded (gas consumed: {})", gas_consumed);
            println!("{}\tgas_consumed\t{}", package, gas_consumed);
        })
    });
    result.assert_success();

    // Print out the transaction's event log
    {
        use groundhog_erc20_if::*;
        use sdk::EmittedEvent;

        for (i, event) in result.events().iter().enumerate() {
            let event: EmittedEvent = event.into();
            match ERC20Event::try_from(event) {
                Ok(event) => eprintln!("\tevent {:?}: {}", i, event),
                Err(_) => eprintln!("\tevent {:?}: <non-ERC20 event>", i),
            }
        }
    }
```

- [ ] **Step 5: Verify**

Run: `cargo build -p bottle-test && cargo test -p erc20-native-test -p erc20-test -p groundhog-erc20-if` (adjust crate names from Cargo.tomls). The end-to-end consumer is `integration_test/contracts/itc_erc20/tests/check.rs`, which needs no edits; run it via the integration_test workspace if it builds locally: `cargo test --manifest-path integration_test/Cargo.toml -p itc_erc20` (skip with a note if the submodule workspace isn't independently buildable).
Expected: PASS.

- [ ] **Step 6: Commit** (excludes integration_test)

```bash
git add standard-interfaces standard-tests crates/bottle-test
git commit -m "standard-tests/bottle-test: migrate harnesses to env.transact; XDR derives for StdERC20Event"
```

---

### Task 8: integration_test/contracts edits (NO COMMIT — Geoff commits the submodule)

**Files:**
- Modify: `integration_test/contracts/calldata_wrapper/src/tests.rs`
- Modify: `integration_test/contracts/read_only_test/src/tests.rs`
- Modify: `integration_test/contracts/fibonacci_call/tests/native.rs`
- Modify: `integration_test/contracts/deploy_and_transfer/src/lib.rs`
- Modify: `integration_test/contracts/simple_wallet/src/lib.rs`

(`itc_erc20`, `tuple_return`, `ed25519_verify_wrapper`, `fibonacci`, `sha256_wrapper` use `client_test` or `test_erc20_contract`, which survive — no edits.)

- [ ] **Step 1: `calldata_wrapper/src/tests.rs`** — replace the `prepare_test` block:

```rust
#[test]
fn test_calldata_wrapper() {
    let env = sdk::TestEnvironment::new(None);

    let test_contract = env
        .register_mock(<CalldataWrapperContract as CalldataWrapper>::__mock);
    let contract_hash = test_contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("calldata_wrapper")).run(move || {
            test_calldata_wrapper_inner(&contract_hash)
        })
    })
    .assert_success();
}
```

(`test_calldata_wrapper_inner` unchanged; add `use sdk::Named;` if the glob doesn't cover it.)

- [ ] **Step 2: `read_only_test/src/tests.rs`** — same recipe replacing `contract_exec(env, move || ...).unwrap()` with `env.transact(|tx| tx.call(&ContractAddress::named("read_only")).run(move || ...)).assert_success();` (inner fn unchanged).

- [ ] **Step 3: `fibonacci_call/tests/native.rs`** — first test: `contract_exec` recipe from Task 5. Second test (`call_inner_out_of_gas_aborts_cleanly`): replace the `contract_test` + `result.is_err()` check with:

```rust
    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let fibonacci_addr =
                sdk::deploy_contract(&FIBONACCI_NONCE, &fibonacci_hash);
            let caller_addr = sdk::deploy_contract(&CALLER_NONCE, &caller_hash);

            let client = fibonacci_call::Client::new(&caller_addr);
            client.initialize(&fibonacci_addr);

            // call (not try_call) will abort the caller on inner OOG.
            client.call_burn_gas(1000);
        })
    });

    // The caller should have aborted, not crashed.
    result.assert_abort();
```

- [ ] **Step 4: `deploy_and_transfer` and `simple_wallet` — linkme conversion.** Their `#[cfg(feature = "dev")] pub fn register(env) -> Contract` helpers manually list `__mock` fragments. Replace each with the auto-registry:

```rust
sdk::declare_mock_contract!();
```

at module scope (this generates `register_mock_contract(&env) -> MockContract`, assembling ALL `#[contract]` impls in the crate via the linkme registry — including simple_wallet's two trait impls). Then `grep -rn "::register(" integration_test --include="*.rs"` for callers of the old `register` fns and update them to `register_mock_contract(&env)` (callers get a `MockContract`; use `.as_contract()` where a `Contract` value is genuinely needed, or `.as_hash()` for deployment). If grep finds no callers, delete the dead `register` fns outright.

- [ ] **Step 5: Verify and STOP — do not commit**

Run: `cargo test --manifest-path integration_test/Cargo.toml` (or per-crate `-p` if the workspace manifest differs; if the submodule isn't independently buildable in this checkout, run `cargo check` on the touched crates and note it).
Expected: PASS / clean check.
Leave the submodule dirty; report the changed file list in the task report.

---

### Task 9: Delete the legacy API

**Files:**
- Modify: `crates/sdk/src/mock/contract_test.rs`
- Modify: `crates/native-exec/src/test_environment.rs`

Only start this task after Tasks 4–8 are complete (every in-tree user is migrated).

- [ ] **Step 1: Delete from `crates/sdk/src/mock/contract_test.rs`:** `contract_run`, `contract_exec`, `contract_test` (the *function* — the `contract_test!` macro stays), `contract_test_raw`, `contract_test_handler`, `contract_handler_exec`, `contract_handler_exec_raw`, and `grant_storage_tokens` if it has no remaining users (`grep -rn "grant_storage_tokens" --include="*.rs" . | grep -v target` — if used, keep it). `test_wasm`, `load_wasm_contract`, `deploy_address`, `Stats`, `ContractTest`, `client_test`, `into_root_call_result`, `TestRegisterFn` stay. Add a doc comment at the top of the file stating the canonical pattern:

```rust
//! Test sugar over the fluent mock-execution API.
//!
//! The canonical test pattern is:
//! ```rust,ignore
//! let env = TestEnvironment::no_cost();
//! let result = env.transact(|tx| tx.call(&addr).run(|| { /* contract code */ }));
//! result.assert_success();
//! ```
//! `contract_test!` and `client_test` are thin sugar over that pattern.
//! `test_wasm` is the harness for running real wasm blobs through the full
//! SignedTransaction path (the only place gas_consumed is observable).
```

- [ ] **Step 2: Relocate `prepare_test`, then delete it from native-exec.** Note `test_wasm` (which is kept) currently routes through `env.prepare_test`, so the logic moves rather than dies: copy the body of `TestEnvironment::prepare_test` (`crates/native-exec/src/test_environment.rs:474-522`) into `crates/sdk/src/mock/contract_test.rs` as a private helper used only by `test_wasm`:

```rust
/// Build a one-shot TransactionHandler that runs `test` as a mock contract
/// method through the full SignedTransaction path. Internal harness for
/// `test_wasm` — everything else uses `env.transact`.
fn wasm_test_handler<F>(env: &TestEnvironmentHandle, test: F) -> TransactionHandler
where
    F: Fn() + UnwindSafe + Clone + 'static,
{
    // ... verbatim body of the old TestEnvironment::prepare_test, using
    // public native-exec API: env.register_mock / MockContract::register_method,
    // env.setup_create_deployment, TransactionHandler::new(env.clone(), &stx) ...
}
```

The old body uses `env.add_contract()` + `register_method` + `setup_create_contract` + `setup_create_deployment` — all callable from sdk (`add_contract` stays `pub` but doc-hidden, next step). Update `test_wasm` to call `wasm_test_handler(&env, ...)`. Then delete `prepare_test` from native-exec.

- [ ] **Step 3: Doc-hide `add_contract`** in `test_environment.rs` (it cannot be `pub(crate)`: `sdk::mock_registry` and the new `wasm_test_handler` call it from another crate):

```rust
    /// Internal: create an empty mock contract. Use `register_mock` (or the
    /// generated `register_mock_contract`) instead.
    #[doc(hidden)]
    pub fn add_contract(self: &TestEnvironmentHandle) -> MockContract {
```

- [ ] **Step 4: Straggler sweep**

Run: `grep -rn "contract_exec\|contract_test_raw\|contract_test_handler\|contract_handler_exec\|contract_run\|prepare_test" --include="*.rs" . | grep -v target | grep -v docs/`
Expected: zero hits outside this plan/spec. Also `grep -rn "contract_test(" --include="*.rs" . | grep -v "contract_test!"` for stale function-form calls.

- [ ] **Step 5: Verify workspace**

Run: `cargo build --workspace && cargo test --workspace`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/sdk crates/native-exec
git commit -m "sdk/native-exec: delete legacy test wrapper API

contract_run/contract_exec/contract_test(fn)/contract_test_raw/
contract_test_handler/contract_handler_exec{,_raw} and
TestEnvironment::prepare_test are gone; the canonical pattern is
env.transact(|tx| tx.call(&addr).run(..)) with contract_test!/client_test
as the only sugar. test_wasm remains the wasm-blob harness."
```

---

### Task 10: Final verification sweep

- [ ] **Step 1: Full workspace test**

Run: `cargo test --workspace 2>&1 | tail -20`
Expected: all suites pass, zero failures.

- [ ] **Step 2: Lints and formatting**

Run: `cargo fmt --all && cargo clippy --workspace --tests 2>&1 | grep -E "^(warning|error)" | head -20`
Expected: no new warnings introduced by this change (pre-existing warnings are out of scope). Commit any fmt diffs.

- [ ] **Step 3: Doc references** — sweep for prose mentioning deleted functions:

Run: `grep -rn "contract_exec\|contract_test_handler\|prepare_test" README.md crates/*/README.md crates/sdk/src --include="*.md" 2>/dev/null; grep -rn "contract_exec" crates/sdk/src --include="*.rs" | grep "///"`
Update any doc comments/examples found to the canonical pattern.

- [ ] **Step 4: Commit any remaining fixes**

```bash
git add -A ':!integration_test' ':!docs/superpowers'
git commit -m "docs/fmt: finish test-pattern standardization sweep"
```

---

## Out of scope
- `logging-test/` — surveyed; it does not use the test framework at all. No changes.
- `groundhog-examples` (sibling repo, registry-pinned) — migrates on its next SDK upgrade.
- Publishing/version bumps for `sdk`, `groundhog-native-exec`, `groundhog-erc20-if` — release process handles them (breaking change: removed public API).
- The `integration_test` submodule pointer bump — Geoff commits it.
