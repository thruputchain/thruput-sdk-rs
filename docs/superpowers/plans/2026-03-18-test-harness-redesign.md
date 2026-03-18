# Test Harness Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an ergonomic fluent API for contract testing on top of MockTransactionHandler, replacing bool returns with FrameAction enums, adding typed result wrappers with assertion methods, and wiring up event access.

**Architecture:** New modules in the `native_exec` crate provide `FrameResult`, `TransactResult`, `FrameAction`, and builder types (`FrameBuilder`, `ContinuableFrame`, `Transaction`). These wrap the existing `MockTransactionHandler` and `MockInvocationFrame` without changing the FFI layer. `TestEnvironmentHandle` gets new convenience methods (`transact`, `transact_manual`). The SDK crate re-exports via a `testing` module.

**Tech Stack:** Rust, groundhog-sys FFI bindings, groundhog-xdr types

**Spec:** `docs/superpowers/specs/2026-03-18-test-harness-redesign.md`

---

## File Structure

### New files in `native_exec/src/`:

| File | Responsibility |
|------|---------------|
| `frame_result.rs` | `FrameResult` struct — wraps `InvocationFrameOutput` + trace, provides `assert_success/abort/fault`, buffer accessors |
| `transact_result.rs` | `TransactResult` struct — wraps `FrameResult` + `ExecResult` + events, provides transaction-level assertions |
| `frame_action.rs` | `FrameAction` enum (`Continue`/`Close`) |
| `frame_builder.rs` | `FrameBuilder` (replaces `MockInvocationFrameConfig` for new API) and `ContinuableFrame` types |
| `transaction.rs` | `Transaction` struct — wraps `MockTransactionHandler`, provides `call()` method |
| `testing.rs` | `transact()` and `transact_manual()` methods on `TestEnvironmentHandle` |

### Modified files:

| File | Changes |
|------|---------|
| `native_exec/src/lib.rs` | Add `pub mod` for new modules, re-exports |
| `native_exec/src/mock_transaction_handler.rs` | Fix state machine transitions, add `get_event_logs()`, make some fields `pub(crate)` |
| `native_exec/src/mock_contract.rs` | No changes needed |
| `native_exec/src/test_environment.rs` | No changes needed (transact methods go in `testing.rs` as trait impl or free fns on the handle type) |

### Test files:

| File | Purpose |
|------|---------|
| `native_exec/tests/fluent_api.rs` | Integration tests for the new API: success/abort/fault assertions, event access, continuable frames, nested frames |

---

## Task 1: FrameAction Enum

**Files:**
- Create: `native_exec/src/frame_action.rs`
- Modify: `native_exec/src/lib.rs`

- [ ] **Step 1: Create the FrameAction enum**

```rust
// native_exec/src/frame_action.rs

/// Replaces the bool return from MockInvocationFrame::execute().
/// Continue = keep frame open (was: true)
/// Close = close frame (was: false)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAction {
    Continue,
    Close,
}

impl FrameAction {
    /// Convert to the bool expected by MockInvocationFrame::execute()
    pub(crate) fn as_bool(self) -> bool {
        match self {
            FrameAction::Continue => true,
            FrameAction::Close => false,
        }
    }
}
```

- [ ] **Step 2: Register the module in lib.rs**

Add to `native_exec/src/lib.rs`:

```rust
pub mod frame_action;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p groundhog-native-exec`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```
git add native_exec/src/frame_action.rs native_exec/src/lib.rs
git commit -m "feat(native_exec): add FrameAction enum replacing bool returns"
```

---

## Task 2: FrameResult Type

**Files:**
- Create: `native_exec/src/frame_result.rs`
- Modify: `native_exec/src/lib.rs`

- [ ] **Step 1: Write test for FrameResult construction and assertions**

Create `native_exec/tests/fluent_api.rs`:

```rust
use groundhog_native_exec::frame_result::FrameResult;
use groundhog_xdr::transaction::InvocationFrameOutput;

#[test]
fn test_frame_result_success() {
    // Construct a "success" InvocationFrameOutput
    let return_data = vec![1u8, 2, 3];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeReturn {
        return_buffer: return_data.clone().try_into().unwrap(),
    };
    let result = FrameResult::new(output, None);

    assert!(result.is_success());
    assert!(!result.is_abort());
    assert!(!result.is_fault());
    result.assert_success();
    assert_eq!(result.return_buffer(), &[1, 2, 3]);
}

#[test]
fn test_frame_result_abort() {
    let abort_data = vec![4u8, 5, 6];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeAbort {
        abort_buffer: abort_data.clone().try_into().unwrap(),
    };
    let result = FrameResult::new(output, None);

    assert!(result.is_abort());
    assert!(!result.is_success());
    result.assert_abort();
    assert_eq!(result.abort_buffer(), &[4, 5, 6]);
}

#[test]
fn test_frame_result_fault() {
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeFault;
    let result = FrameResult::new(output, None);

    assert!(result.is_fault());
    result.assert_fault();
}

#[test]
#[should_panic(expected = "expected success")]
fn test_frame_result_assert_success_panics_on_fault() {
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeFault;
    let result = FrameResult::new(output, None);
    result.assert_success();
}

#[test]
#[should_panic(expected = "expected abort")]
fn test_frame_result_assert_abort_panics_on_success() {
    let return_data = vec![];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeReturn {
        return_buffer: return_data.try_into().unwrap(),
    };
    let result = FrameResult::new(output, None);
    result.assert_abort();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p groundhog-native-exec --test fluent_api`
Expected: FAIL — `frame_result` module does not exist

- [ ] **Step 3: Implement FrameResult**

```rust
// native_exec/src/frame_result.rs

use groundhog_trace::InvocationFrameTrace;
use groundhog_xdr::transaction::InvocationFrameOutput;
use groundhog_xdr::transaction::InvocationFrameStatusCode;

/// Result of executing a single invocation frame.
/// Wraps InvocationFrameOutput + optional trace.
pub struct FrameResult {
    output: InvocationFrameOutput,
    trace: Option<InvocationFrameTrace>,
}

impl FrameResult {
    pub fn new(
        output: InvocationFrameOutput,
        trace: Option<InvocationFrameTrace>,
    ) -> Self {
        Self { output, trace }
    }

    // -- Status checks (using pattern matching for safety) --

    pub fn is_success(&self) -> bool {
        matches!(
            &self.output,
            InvocationFrameOutput::kInvocationFrameStatusCodeReturn { .. }
        )
    }

    pub fn is_abort(&self) -> bool {
        matches!(
            &self.output,
            InvocationFrameOutput::kInvocationFrameStatusCodeAbort { .. }
        )
    }

    pub fn is_fault(&self) -> bool {
        matches!(
            &self.output,
            InvocationFrameOutput::kInvocationFrameStatusCodeFault
        )
    }

    // -- Assertions --

    pub fn assert_success(&self) {
        assert!(
            self.is_success(),
            "expected success, got status code {}",
            self.output.status_code()
        );
    }

    pub fn assert_abort(&self) {
        assert!(
            self.is_abort(),
            "expected abort, got status code {}",
            self.output.status_code()
        );
    }

    pub fn assert_fault(&self) {
        assert!(
            self.is_fault(),
            "expected fault, got status code {}",
            self.output.status_code()
        );
    }

    // -- Buffer accessors --

    /// Returns the return buffer. Panics if the frame did not succeed.
    pub fn return_buffer(&self) -> &[u8] {
        match &self.output {
            InvocationFrameOutput::kInvocationFrameStatusCodeReturn {
                return_buffer,
            } => return_buffer.as_slice(),
            _ => panic!(
                "return_buffer() called on non-success frame (status {})",
                self.output.status_code()
            ),
        }
    }

    /// Returns the abort buffer. Panics if the frame did not abort.
    pub fn abort_buffer(&self) -> &[u8] {
        match &self.output {
            InvocationFrameOutput::kInvocationFrameStatusCodeAbort {
                abort_buffer,
            } => abort_buffer.as_slice(),
            _ => panic!(
                "abort_buffer() called on non-abort frame (status {})",
                self.output.status_code()
            ),
        }
    }

    // -- Typed abort assertion --

    /// Assert the frame aborted and decode the abort buffer as a typed error.
    /// Uses the Exn trait from the SDK for decoding.
    /// The check closure receives the decoded error for further assertions.
    pub fn assert_abort_with<E>(&self, check: impl FnOnce(&E))
    where
        E: xdrs::ReadXDROwned,
    {
        self.assert_abort();
        let buf = self.abort_buffer();
        let decoded: E = xdrs::txs::decode(buf)
            .unwrap_or_else(|e| panic!("failed to decode abort buffer: {}", e));
        check(&decoded);
    }

    // -- Raw access --

    pub fn output(&self) -> &InvocationFrameOutput {
        &self.output
    }

    pub fn trace(&self) -> Option<&InvocationFrameTrace> {
        self.trace.as_ref()
    }

    // -- Per-frame events (TODO) --

    /// Per-frame event access is not yet supported.
    /// Requires adding events to InvocationFrameTrace in hogd.
    /// Use TransactResult::events() for transaction-level events.
    pub fn events(&self) -> &[groundhog_xdr::event::TxEmittedEvent] {
        todo!(
            "per-frame event access not yet supported — \
             use TransactResult.events() for transaction-level events"
        )
    }
}
```

- [ ] **Step 4: Register module in lib.rs**

Add to `native_exec/src/lib.rs`:

```rust
pub mod frame_result;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p groundhog-native-exec --test fluent_api`
Expected: all 5 tests PASS

- [ ] **Step 6: Commit**

```
git add native_exec/src/frame_result.rs native_exec/src/lib.rs native_exec/tests/fluent_api.rs
git commit -m "feat(native_exec): add FrameResult with status checks and assertions"
```

---

## Task 3: TransactResult Type

**Files:**
- Create: `native_exec/src/transact_result.rs`
- Modify: `native_exec/src/lib.rs`
- Modify: `native_exec/tests/fluent_api.rs`

- [ ] **Step 1: Write test for TransactResult**

Append to `native_exec/tests/fluent_api.rs`:

```rust
use groundhog_native_exec::transact_result::TransactResult;
use groundhog_xdr::exec_result::ExecResult;

#[test]
fn test_transact_result_success() {
    let return_data = vec![];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeReturn {
        return_buffer: return_data.try_into().unwrap(),
    };
    let frame = FrameResult::new(output, None);
    let result = TransactResult::new(frame, ExecResult::ExecResultOK, None);

    result.assert_success();
}

#[test]
fn test_transact_result_abort() {
    let abort_data = vec![];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeAbort {
        abort_buffer: abort_data.try_into().unwrap(),
    };
    let frame = FrameResult::new(output, None);
    // hogd returns ExecResultTxRootFrameReverted when root frame aborts
    let result = TransactResult::new(
        frame,
        ExecResult::ExecResultTxRootFrameReverted,
        None,
    );

    result.assert_abort();
}

#[test]
fn test_transact_result_commit_failed() {
    let return_data = vec![];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeReturn {
        return_buffer: return_data.try_into().unwrap(),
    };
    let frame = FrameResult::new(output, None);
    let result = TransactResult::new(
        frame,
        ExecResult::ExecResultTxDeterministicError,
        None,
    );

    result.assert_commit_failed();
}

#[test]
#[should_panic(expected = "expected success")]
fn test_transact_result_assert_success_panics_on_abort() {
    let abort_data = vec![];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeAbort {
        abort_buffer: abort_data.try_into().unwrap(),
    };
    let frame = FrameResult::new(output, None);
    let result = TransactResult::new(
        frame,
        ExecResult::ExecResultTxRootFrameReverted,
        None,
    );
    result.assert_success();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p groundhog-native-exec --test fluent_api`
Expected: FAIL — `transact_result` module does not exist

- [ ] **Step 3: Implement TransactResult**

```rust
// native_exec/src/transact_result.rs

use crate::frame_result::FrameResult;
use groundhog_xdr::event::{TxEmittedEvent, TxEventLog};
use groundhog_xdr::exec_result::ExecResult;

/// Result of a full transaction: root frame result + commit outcome + events.
pub struct TransactResult {
    frame: FrameResult,
    exec_result: ExecResult,
    event_log: Option<TxEventLog>,
}

impl TransactResult {
    pub fn new(
        frame: FrameResult,
        exec_result: ExecResult,
        event_log: Option<TxEventLog>,
    ) -> Self {
        Self {
            frame,
            exec_result,
            event_log,
        }
    }

    // -- Delegates to FrameResult --

    /// Asserts the root frame returned successfully AND the transaction committed OK.
    pub fn assert_success(&self) {
        self.frame.assert_success();
        assert_eq!(
            self.exec_result,
            ExecResult::ExecResultOK,
            "expected successful commit, got {:?}",
            self.exec_result
        );
    }

    pub fn assert_abort(&self) {
        self.frame.assert_abort();
    }

    pub fn assert_fault(&self) {
        self.frame.assert_fault();
    }

    pub fn assert_abort_with<E>(&self, check: impl FnOnce(&E))
    where
        E: xdrs::ReadXDROwned,
    {
        self.frame.assert_abort_with(check);
    }

    // -- Transaction-level assertions --

    /// Asserts the root frame succeeded but the DB commit step failed.
    pub fn assert_commit_failed(&self) {
        self.frame.assert_success();
        assert_ne!(
            self.exec_result,
            ExecResult::ExecResultOK,
            "expected commit failure, but commit succeeded"
        );
    }

    pub fn exec_result(&self) -> &ExecResult {
        &self.exec_result
    }

    pub fn frame(&self) -> &FrameResult {
        &self.frame
    }

    // -- Events --

    pub fn events(&self) -> &[TxEmittedEvent] {
        match &self.event_log {
            Some(log) => &log.events,
            None => &[],
        }
    }

    pub fn assert_event(
        &self,
        index: usize,
        check: impl FnOnce(&TxEmittedEvent),
    ) {
        let events = self.events();
        assert!(
            index < events.len(),
            "event index {} out of range (have {} events)",
            index,
            events.len()
        );
        check(&events[index]);
    }

    /// Decode event at `index` as type E and pass to check closure.
    /// E must implement a conversion from TxEmittedEvent (e.g. via the
    /// SDK's EmittedEvent type and TryInto).
    ///
    /// Note: The exact trait bound here depends on how the SDK's event
    /// decoding works. The implementer should check `src/event.rs` for
    /// the EmittedEvent type and adjust the bound accordingly.
    /// A simple first pass can just decode the serialized_event payload
    /// via ReadXDROwned, skipping topic bytes.
    pub fn assert_event_data<E>(
        &self,
        index: usize,
        check: impl FnOnce(&E),
    ) where
        E: xdrs::ReadXDROwned,
    {
        let events = self.events();
        assert!(
            index < events.len(),
            "event index {} out of range (have {} events)",
            index,
            events.len()
        );
        let event = &events[index];
        let topics_len = 32 * event.num_topics as usize;
        let payload = &event.serialized_event[topics_len..];
        let decoded: E = xdrs::txs::decode(payload)
            .unwrap_or_else(|e| panic!("failed to decode event data: {}", e));
        check(&decoded);
    }

    /// Assert that any event in the log matches the predicate when decoded as E.
    pub fn assert_emitted<E>(
        &self,
        predicate: impl Fn(&E) -> bool,
    ) where
        E: xdrs::ReadXDROwned,
    {
        let events = self.events();
        for event in events {
            let topics_len = 32 * event.num_topics as usize;
            let payload = &event.serialized_event[topics_len..];
            if let Ok(decoded) = xdrs::txs::decode::<E>(payload) {
                if predicate(&decoded) {
                    return;
                }
            }
        }
        panic!(
            "no event matched the predicate (checked {} events)",
            events.len()
        );
    }
}
```

- [ ] **Step 4: Register module in lib.rs**

Add to `native_exec/src/lib.rs`:

```rust
pub mod transact_result;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p groundhog-native-exec --test fluent_api`
Expected: all tests PASS

- [ ] **Step 6: Commit**

```
git add native_exec/src/transact_result.rs native_exec/src/lib.rs native_exec/tests/fluent_api.rs
git commit -m "feat(native_exec): add TransactResult with commit and event assertions"
```

---

## Task 4: Fix MockTransactionHandler State Machine

**Files:**
- Modify: `native_exec/src/mock_transaction_handler.rs`

This is a prerequisite for the `transact()` wrapper — commit/abort/events all depend on correct state transitions.

- [ ] **Step 1: Write test for state machine transitions**

Append to `native_exec/tests/fluent_api.rs`:

```rust
use groundhog_native_exec::mock_transaction_handler::MockTransactionHandler;
use groundhog_native_exec::test_environment::TestEnvironment;
use groundhog_xdr::exec_result::ExecResult;

#[test]
fn test_mock_handler_commit() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    handler
        .open_root_frame()
        .with_invoke_flags(0)
        .build()
        .execute(|_| false)
        .result();

    // Should be able to commit after root frame closes
    let exec_result = handler.try_commit_transaction();
    assert_eq!(exec_result, ExecResult::ExecResultOK);
}

#[test]
fn test_mock_handler_abort() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    handler
        .open_root_frame()
        .with_invoke_flags(0)
        .build()
        .execute(|_| false)
        .result();

    // Should be able to abort after root frame closes
    handler.abort_transaction();
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p groundhog-native-exec --test fluent_api test_mock_handler`
Expected: FAIL — assertion `self.state == MockTransactionState::RootInvokeFinished` fails because state is still `NotStarted`

- [ ] **Step 3: Fix state transitions in MockTransactionHandler**

In `native_exec/src/mock_transaction_handler.rs`:

1. In `open_root_frame()` (line 378), add state transition and guard:

```rust
pub fn open_root_frame(&mut self) -> MockInvocationFrameConfig<'_> {
    assert_eq!(
        self.state,
        MockTransactionState::NotStarted,
        "cannot open root frame: transaction already started"
    );
    self.state = MockTransactionState::Running;
    let config = MockInvocationFrameConfig::new(
        self.handler,
        &mut self.root_frame,
        self.context_pushcount.clone(),
    );
    config
}
```

2. Auto-transition to `RootInvokeFinished` when the root frame closes. The `MockInvocationFrame` doesn't know if it's the root, so we use a callback. Add a `notify_root_frame_closed` method on `MockTransactionHandler`:

```rust
pub fn notify_root_frame_closed(&mut self) {
    if self.state == MockTransactionState::Running {
        self.state = MockTransactionState::RootInvokeFinished;
    }
}
```

3. **Critical for backwards compatibility:** Update the `Drop` impl to handle `Running` state by auto-transitioning to `RootInvokeFinished` before aborting. This ensures existing tests (which never call commit/abort) don't panic:

```rust
impl Drop for MockTransactionHandler {
    fn drop(&mut self) {
        // Auto-transition: if we're still Running, the root frame must have
        // been closed (frames panic on drop if still Started), so we can
        // safely transition to RootInvokeFinished.
        if self.state == MockTransactionState::Running {
            self.state = MockTransactionState::RootInvokeFinished;
        }
        if self.state == MockTransactionState::RootInvokeFinished {
            eprintln!(
                "Cleanup: Aborting mock transaction that finished root_invoke but was not committed"
            );
            self.abort_transaction();
        }

        let err: FFIErrorCode =
            unsafe { groundhog_sys::close_syscall_handler(self.handler) }
                .into();
        if err != FFIErrorCode::None {
            panic!("failed to close syscall handler: {:?}", err);
        }
    }
}
```

This Drop change is backwards compatible: existing tests open a frame, execute, call result(), then drop the handler. With the fix, state goes `NotStarted` → `Running` (in `open_root_frame`) → auto-transitions to `RootInvokeFinished` (in Drop since frame is closed) → aborts (cleanup) → closes handler.

The `transact()` wrapper (Task 6) will call `notify_root_frame_closed()` explicitly before `try_commit_transaction()`, so it won't rely on the Drop fallback.

Update the test to also call `notify_root_frame_closed()` explicitly (the explicit path):

```rust
#[test]
fn test_mock_handler_commit() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    handler
        .open_root_frame()
        .with_invoke_flags(0)
        .build()
        .execute(|_| false)
        .result();

    handler.notify_root_frame_closed();
    let exec_result = handler.try_commit_transaction();
    assert_eq!(exec_result, ExecResult::ExecResultOK);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p groundhog-native-exec --test fluent_api test_mock_handler`
Expected: both tests PASS

- [ ] **Step 5: Verify existing tests still pass**

Run: `cargo test -p groundhog-native-exec`
Expected: all existing tests PASS. The Drop impl auto-transitions `Running` → `RootInvokeFinished` → abort, so existing tests that never called commit/abort continue to work.

- [ ] **Step 6: Commit**

```
git add native_exec/src/mock_transaction_handler.rs native_exec/tests/fluent_api.rs
git commit -m "fix(native_exec): add state transitions to MockTransactionHandler"
```

---

## Task 5: Add get_event_logs() to MockTransactionHandler

**Files:**
- Modify: `native_exec/src/mock_transaction_handler.rs`
- Modify: `native_exec/tests/fluent_api.rs`

- [ ] **Step 1: Write test for event log access**

Append to `native_exec/tests/fluent_api.rs`:

```rust
#[test]
fn test_mock_handler_event_logs_empty() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    handler
        .open_root_frame()
        .with_invoke_flags(0)
        .build()
        .execute(|_| false)
        .result();

    handler.notify_root_frame_closed();
    handler.try_commit_transaction();

    // No events emitted, should return None or empty
    let events = handler.get_event_logs();
    assert!(events.is_none() || events.unwrap().events.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p groundhog-native-exec --test fluent_api test_mock_handler_event`
Expected: FAIL — `get_event_logs` method does not exist on `MockTransactionHandler`

- [ ] **Step 3: Implement get_event_logs on MockTransactionHandler**

Port the implementation from `TransactionHandler::get_event_logs()` (at `native_exec/src/transaction_handler.rs:113-162`). The logic is identical — both types hold a `handler: *mut c_void` syscall handler pointer and the same FFI calls work.

Add to `MockTransactionHandler` impl block in `native_exec/src/mock_transaction_handler.rs`:

```rust
pub fn get_event_logs(&self) -> Option<groundhog_xdr::event::TxEventLog> {
    assert!(
        self.state == MockTransactionState::Committed
            || self.state == MockTransactionState::Aborted,
        "cannot get event logs before transaction is committed or aborted"
    );

    let buffer_len_res =
        unsafe { groundhog_sys::get_event_log_len(self.handler) };
    let err: FFIErrorCode = buffer_len_res.ffi_error_code.into();
    if err == FFIErrorCode::EventLogNotFound {
        return None;
    }
    if err != FFIErrorCode::None {
        panic!("failed to get event log length: {:?}", err);
    }
    let buffer_len: u32 = buffer_len_res.len;
    if buffer_len == 0 {
        return None;
    }

    let mut buffer = vec![0u8; buffer_len as usize];
    let event_log_res = unsafe {
        groundhog_sys::get_event_log(
            self.handler,
            buffer.as_mut_ptr(),
            buffer_len,
        )
    };
    let err: FFIErrorCode = event_log_res.into();
    if err != FFIErrorCode::None {
        panic!("failed to get event log: {:?}", err);
    }

    let decoded: groundhog_xdr::event::TxEventLog =
        xdrs::txs::decode(&buffer).unwrap_or_else(|e| {
            panic!("failed to decode event log ({} bytes): {}", buffer.len(), e)
        });

    Some(decoded)
}
```

You'll need to add `use groundhog_xdr::event::TxEventLog;` to the imports at the top of the file if not already present.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p groundhog-native-exec --test fluent_api test_mock_handler_event`
Expected: PASS

- [ ] **Step 5: Commit**

```
git add native_exec/src/mock_transaction_handler.rs native_exec/tests/fluent_api.rs
git commit -m "feat(native_exec): add get_event_logs() to MockTransactionHandler"
```

---

## Task 6: FrameBuilder and Transaction Types

**Files:**
- Create: `native_exec/src/frame_builder.rs`
- Create: `native_exec/src/transaction.rs`
- Modify: `native_exec/src/lib.rs`
- Modify: `native_exec/src/mock_transaction_handler.rs` (make fields `pub(crate)`)

This is the core fluent API. `Transaction` wraps `MockTransactionHandler` and provides `call()`. `FrameBuilder` wraps the frame config and provides `run()` / `run_continuable()`.

- [ ] **Step 1: Write test for basic transact flow using the new types directly**

Append to `native_exec/tests/fluent_api.rs`:

```rust
use groundhog_native_exec::transaction::Transaction;

#[test]
fn test_transaction_call_run_success() {
    let env = TestEnvironment::new(None);
    let mut tx = Transaction::new(env);

    let frame_result = tx
        .call(&[1u8; 32].into())
        .run(|| {
            // Inside a contract frame — syscalls available
            let res = groundhog_xdr::syscalls::GetCurrentAddress();
            assert_eq!(
                res.unwrap(),
                groundhog_xdr::types::ContractAddress::from([1; 32])
            );
        });

    frame_result.assert_success();
}

#[test]
fn test_transaction_call_run_fault() {
    let env = TestEnvironment::new(None);
    let mut tx = Transaction::new(env);

    let frame_result = tx
        .call(&[1u8; 32].into())
        .run(|| {
            panic!("intentional test fault");
        });

    frame_result.assert_fault();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p groundhog-native-exec --test fluent_api test_transaction_call`
Expected: FAIL — `transaction` module does not exist

- [ ] **Step 3: Make MockTransactionHandler fields accessible**

In `native_exec/src/mock_transaction_handler.rs`, change the `MockTransactionHandler` struct fields that the `Transaction` wrapper needs:

```rust
pub struct MockTransactionHandler {
    pub(crate) handler: *mut c_void,
    pub(crate) _env: Rc<TestEnvironment>,
    pub(crate) state: MockTransactionState,
    pub(crate) root_frame: Option<Box<MockInvocationFrame>>,
    pub(crate) context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
}
```

Also make `MockTransactionState` pub(crate):

```rust
#[derive(Debug, PartialEq)]
pub(crate) enum MockTransactionState { ... }
```

- [ ] **Step 4: Implement FrameBuilder**

```rust
// native_exec/src/frame_builder.rs

use crate::frame_action::FrameAction;
use crate::frame_result::FrameResult;
use crate::mock_transaction_handler::{
    MockInvocationFrame, MockInvocationFrameConfig,
};
use std::panic::UnwindSafe;

/// Builder for configuring and executing a single invocation frame.
/// Created by Transaction::call() or ContinuableFrame::child_call().
pub struct FrameBuilder<'a> {
    config: MockInvocationFrameConfig<'a>,
}

impl<'a> FrameBuilder<'a> {
    pub(crate) fn new(config: MockInvocationFrameConfig<'a>) -> Self {
        Self { config }
    }

    pub fn with_calldata(
        self,
        calldata: xdr_common::BytesM,
    ) -> Self {
        Self {
            config: self.config.with_calldata(calldata),
        }
    }

    pub fn with_flags(self, flags: i32) -> Self {
        Self {
            config: self.config.with_invoke_flags(flags),
        }
    }

    /// Execute the frame with the given closure, then close it.
    /// This is the common single-shot path.
    pub fn run(self, f: impl FnOnce() + UnwindSafe) -> FrameResult {
        let frame = self.config.build();
        frame.execute(|_| {
            f();
            false // always close
        });
        let (output, trace) = frame.result();
        FrameResult::new(output, trace)
    }

    /// Execute the frame with a closure that receives a ContinuableFrame
    /// and returns a FrameAction.
    pub fn run_continuable(
        self,
        f: impl FnOnce(&mut ContinuableFrame) -> FrameAction + UnwindSafe,
    ) -> ContinuableFrame<'a> {
        let frame = self.config.build();

        let action = {
            let mut cf = ContinuableFrame { frame };
            frame.execute(|inner_frame| {
                // We need to give the closure a ContinuableFrame that
                // wraps the inner MockInvocationFrame
                let mut cf = ContinuableFrame { frame: inner_frame };
                let action = f(&mut cf);
                action.as_bool()
            });
            cf
        };

        // TODO: The lifetime management here is tricky because
        // MockInvocationFrame::execute takes FnOnce(&mut Self) -> bool.
        // The ContinuableFrame needs to wrap the frame reference from
        // the execute callback. This requires careful implementation
        // that works within execute's closure model.
        //
        // For the initial implementation, we can use a simpler approach:
        // run_continuable calls execute() and checks if the frame is still open.
        todo!("implement after verifying basic run() works")
    }
}

/// Wraps a MockInvocationFrame that is still open (FrameAction::Continue).
/// Supports chaining additional run_continuable() calls and child_call().
pub struct ContinuableFrame<'a> {
    frame: &'a mut MockInvocationFrame,
}

impl<'a> ContinuableFrame<'a> {
    pub fn child_call(
        &mut self,
        addr: &groundhog_xdr::types::ContractAddress,
    ) -> FrameBuilder<'_> {
        let config = self
            .frame
            .open_child_frame()
            .at_contract_address(*addr);
        FrameBuilder::new(config)
    }

    pub fn run_continuable(
        self,
        f: impl FnOnce(&mut ContinuableFrame) -> FrameAction + UnwindSafe,
    ) -> ContinuableFrame<'a> {
        todo!("continuable chaining")
    }

    pub fn finish(self) -> FrameResult {
        let (output, trace) = self.frame.result();
        FrameResult::new(output, trace)
    }
}
```

Note: The `run_continuable` / `ContinuableFrame` implementation requires careful lifetime management around `MockInvocationFrame::execute`'s closure model. The initial implementation should focus on getting `run()` working. Mark `run_continuable` and `ContinuableFrame::run_continuable` as `todo!()` — they can be implemented as a follow-up once the basic flow works. The tests in this task only exercise `run()`.

- [ ] **Step 5: Implement Transaction**

```rust
// native_exec/src/transaction.rs

use crate::frame_builder::FrameBuilder;
use crate::frame_result::FrameResult;
use crate::mock_transaction_handler::MockTransactionHandler;
use crate::test_environment::TestEnvironmentHandle;
use crate::transact_result::TransactResult;
use groundhog_xdr::exec_result::ExecResult;
use groundhog_xdr::types::ContractAddress;

/// Handle to a mock transaction in progress.
/// Created by TestEnvironment::transact() or transact_manual().
pub struct Transaction {
    handler: MockTransactionHandler,
}

impl Transaction {
    pub fn new(env: TestEnvironmentHandle) -> Self {
        Self {
            handler: MockTransactionHandler::new(env),
        }
    }

    /// Begin a root frame call to the given contract address.
    /// Returns a FrameBuilder for configuring and executing the frame.
    pub fn call(&mut self, addr: &ContractAddress) -> FrameBuilder<'_> {
        let config = self
            .handler
            .open_root_frame()
            .at_contract_address(*addr);
        FrameBuilder::new(config)
    }

    /// Commit the transaction. Returns the ExecResult.
    pub fn commit(&mut self) -> ExecResult {
        self.handler.notify_root_frame_closed();
        self.handler.try_commit_transaction()
    }

    /// Abort the transaction.
    pub fn abort(&mut self) {
        self.handler.notify_root_frame_closed();
        self.handler.abort_transaction();
    }

    /// Get event logs after commit/abort.
    pub(crate) fn get_event_logs(
        &self,
    ) -> Option<groundhog_xdr::event::TxEventLog> {
        self.handler.get_event_logs()
    }
}
```

- [ ] **Step 6: Register modules in lib.rs**

Add to `native_exec/src/lib.rs`:

```rust
pub mod frame_builder;
pub mod transaction;
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p groundhog-native-exec --test fluent_api test_transaction_call`
Expected: both tests PASS

- [ ] **Step 8: Commit**

```
git add native_exec/src/frame_builder.rs native_exec/src/transaction.rs native_exec/src/mock_transaction_handler.rs native_exec/src/lib.rs native_exec/tests/fluent_api.rs
git commit -m "feat(native_exec): add Transaction and FrameBuilder fluent API"
```

---

## Task 7: transact() and transact_manual() on TestEnvironmentHandle

**Files:**
- Create: `native_exec/src/testing.rs`
- Modify: `native_exec/src/lib.rs`
- Modify: `native_exec/tests/fluent_api.rs`

- [ ] **Step 1: Write integration test using transact()**

Append to `native_exec/tests/fluent_api.rs`:

```rust
use groundhog_native_exec::testing::TestEnvExt;

#[test]
fn test_transact_success() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            let res = groundhog_xdr::syscalls::GetCurrentAddress();
            assert_eq!(
                res.unwrap(),
                groundhog_xdr::types::ContractAddress::from([1; 32])
            );
        })
    });

    result.assert_success();
}

#[test]
fn test_transact_fault() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            panic!("intentional fault");
        })
    });

    result.assert_fault();
}

#[test]
fn test_transact_manual_commit() {
    let env = TestEnvironment::new(None);

    let result = env.transact_manual(|tx| {
        let frame_result = tx.call(&[1u8; 32].into()).run(|| {});
        frame_result.assert_success();
        tx.commit();
    });

    // transact_manual returns the Transaction for further inspection
    // (this test just verifies it doesn't panic)
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p groundhog-native-exec --test fluent_api test_transact`
Expected: FAIL — `testing` module does not exist

- [ ] **Step 3: Implement testing.rs**

```rust
// native_exec/src/testing.rs

use crate::frame_result::FrameResult;
use crate::test_environment::TestEnvironmentHandle;
use crate::transact_result::TransactResult;
use crate::transaction::Transaction;

/// Extension trait adding ergonomic transaction methods to TestEnvironmentHandle.
pub trait TestEnvExt {
    /// Execute a transaction with auto-commit.
    ///
    /// Creates a MockTransactionHandler, gives the closure a Transaction handle,
    /// and always calls try_commit_transaction when the closure returns
    /// (hogd handles abort internally when the root frame reverted).
    fn transact(
        &self,
        f: impl FnOnce(&mut Transaction) -> FrameResult,
    ) -> TransactResult;

    /// Execute a transaction with manual commit/abort control.
    ///
    /// The closure receives a Transaction and is responsible for
    /// calling tx.commit() or tx.abort().
    fn transact_manual(
        &self,
        f: impl FnOnce(&mut Transaction),
    );
}

impl TestEnvExt for TestEnvironmentHandle {
    fn transact(
        &self,
        f: impl FnOnce(&mut Transaction) -> FrameResult,
    ) -> TransactResult {
        let mut tx = Transaction::new(self.clone());

        let frame_result = f(&mut tx);

        let exec_result = tx.commit();
        let event_log = tx.get_event_logs();

        TransactResult::new(frame_result, exec_result, event_log)
    }

    fn transact_manual(
        &self,
        f: impl FnOnce(&mut Transaction),
    ) {
        let mut tx = Transaction::new(self.clone());
        f(&mut tx);
        // Caller is responsible for commit/abort.
        // If neither was called, Transaction's Drop (via MockTransactionHandler::Drop)
        // will handle cleanup.
    }
}
```

- [ ] **Step 4: Register module in lib.rs**

Add to `native_exec/src/lib.rs`:

```rust
pub mod testing;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p groundhog-native-exec --test fluent_api test_transact`
Expected: all 3 tests PASS

- [ ] **Step 6: Run all tests to make sure nothing is broken**

Run: `cargo test -p groundhog-native-exec`
Expected: all tests PASS

- [ ] **Step 7: Commit**

```
git add native_exec/src/testing.rs native_exec/src/lib.rs native_exec/tests/fluent_api.rs
git commit -m "feat(native_exec): add transact() and transact_manual() on TestEnvironmentHandle"
```

---

## Task 8: SDK Re-exports

**Files:**
- Modify: `src/lib.rs` or whichever file currently re-exports native_exec types
- Modify: `native_exec/tests/fluent_api.rs` (optional: add test using sdk paths)

The goal is for SDK users to access the new API via `sdk::testing::*` or similar.

- [ ] **Step 1: Find where native_exec is currently re-exported**

Look at `src/lib.rs` and `src/dev/mod.rs` for existing re-exports of `TestEnvironment`, `TestEnvironmentHandle`, etc. The SDK already re-exports `TestEnvironment::no_cost()` and related types.

- [ ] **Step 2: Add re-exports for new types**

In the appropriate SDK module (likely `src/dev/mod.rs` or `src/lib.rs` under `#[cfg(feature = "mock")]`), add:

```rust
pub use groundhog_native_exec::frame_action::FrameAction;
pub use groundhog_native_exec::frame_result::FrameResult;
pub use groundhog_native_exec::transact_result::TransactResult;
pub use groundhog_native_exec::transaction::Transaction;
pub use groundhog_native_exec::testing::TestEnvExt;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p sdk`
Expected: compiles

- [ ] **Step 4: Commit**

```
git add src/
git commit -m "feat(sdk): re-export new test harness API"
```

---

## Task 9: End-to-End Validation Test

**Files:**
- Modify: `native_exec/tests/fluent_api.rs`

Write a comprehensive test that exercises the full flow from the spec's "Multi-Transaction Scenario Example", adapted to use mock contracts (since we can't use real contract impls in the native_exec crate's tests).

- [ ] **Step 1: Write end-to-end test**

```rust
#[test]
fn test_multi_transaction_scenario() {
    let env = TestEnvironment::new(None);

    // Transaction 1: succeeds
    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            // Contract code that succeeds
        })
    });
    result.assert_success();

    // Advance block
    env.advance_block();

    // Transaction 2: succeeds
    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            // More contract code
        })
    });
    result.assert_success();

    // Transaction 3: faults
    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            panic!("simulated failure");
        })
    });
    result.assert_fault();
}

#[test]
fn test_frame_result_raw_access() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {})
    });

    // Can access raw output
    let _output = result.frame().output();
    let _exec = result.exec_result();
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p groundhog-native-exec --test fluent_api`
Expected: all tests PASS

- [ ] **Step 3: Commit**

```
git add native_exec/tests/fluent_api.rs
git commit -m "test(native_exec): add end-to-end validation tests for fluent API"
```

---

## Implementation Notes for the Implementer

### Key files to read before starting:
- `native_exec/src/mock_transaction_handler.rs` — the engine you're wrapping
- `native_exec/src/test_environment.rs` — where `TestEnvironmentHandle` is defined (line 352: `pub type TestEnvironmentHandle = Rc<TestEnvironment>`)
- `native_exec/src/transaction_handler.rs:113-162` — the `get_event_logs` implementation to port
- `native_exec/src/mock_contract.rs` — understand `MockContract` / `MockContractInner` if you need to extend contract registration

### XDR type locations (these are in external crates, not in this repo):
- `InvocationFrameOutput` — `groundhog_xdr::transaction::InvocationFrameOutput` (enum with three variants: `kInvocationFrameStatusCodeReturn { return_buffer }`, `kInvocationFrameStatusCodeAbort { abort_buffer }`, `kInvocationFrameStatusCodeFault`)
- `InvocationFrameStatusCode` — `groundhog_xdr::transaction::InvocationFrameStatusCode` (enum with i32 discriminants)
- `ExecResult` — `groundhog_xdr::exec_result::ExecResult` (enum: `ExecResultOK`, `ExecResultTxRootFrameReverted`, `ExecResultTxDeterministicError`, etc.)
- `TxEventLog` / `TxEmittedEvent` — `groundhog_xdr::event`
- `ContractAddress` — `groundhog_xdr::types::ContractAddress` (newtype around `[u8; 32]`)

### The `execute()` closure constraint:
`MockInvocationFrame::execute` takes `impl FnOnce(&mut Self) -> bool + UnwindSafe`. The `run()` wrapper needs to wrap the user's `FnOnce()` into `|_: &mut MockInvocationFrame| { f(); false }`. The `UnwindSafe` bound propagates — the user's closure must also be `UnwindSafe`, which is fine because the existing API already requires it.

### Running tests:
These tests require the native hogd library to be linked. Build with:
```
cargo test -p groundhog-native-exec --test fluent_api
```
If the build fails due to missing hogd, ensure `groundhog-sys` can find the native library (check `groundhog-sys/build.rs` for build instructions).

### Contract lifecycle convenience methods are deferred:
The spec defines `add_mock_contract`, `register_contract`, `deploy`, and `deploy_mock<T>` convenience methods on `TestEnvironmentHandle`. These are useful sugar but not required for the core API to work — the existing `env.add_contract()`, `env.setup_create_contract()`, and `env.setup_create_deployment()` methods remain available. These can be added as a follow-up task.

### ContinuableFrame is deferred:
The `run_continuable` / `ContinuableFrame` implementation involves tricky lifetime management because `execute()` gives you `&mut MockInvocationFrame` inside a closure, but `ContinuableFrame` needs to hold that reference across calls. The initial implementation should mark these as `todo!()`. They can be implemented as a follow-up task once the basic `run()` path is validated.
