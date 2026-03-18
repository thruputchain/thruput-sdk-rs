# Test Harness Redesign: Ergonomic Contract Testing API

## Problem

The current test infrastructure has accumulated multiple overlapping APIs:
- `src/contract_test.rs` — older macros (`contract_test!`, `contract_test_env!`) and helpers (`contract_test_invoke`, `contract_test_succeeding`)
- `src/dev/contract_test.rs` — newer macros (`contract_test!`, `client_test!`) and helpers (`contract_test_handler`, `contract_handler_exec`)
- `native_exec/src/mock_transaction_handler.rs` — the underlying frame-level FFI bindings
- `native_exec/src/transaction_handler.rs` — the `SignedTransaction` execution path

Pain points:
- **Bool return in `execute()`** — `true` means "keep frame open", `false` means "close". Non-obvious and error-prone.
- **Arc<Mutex<Option<_>>> dance** — extracting results from contract closures requires shared mutable state workarounds.
- **No failure assertions** — no ergonomic way to assert a call should revert/abort with a specific error.
- **No event assertions** — must manually call `get_event_logs()` and index into raw arrays.
- **Macro-heavy** — `contract_test!` and `client_test!` hide boilerplate but are opaque, hard to debug, and duplicated across two modules.
- **Result inspection requires pattern matching** — every test must match on `kInvocationFrameStatusCodeReturn` / `kInvocationFrameStatusCodeAbort` / `kInvocationFrameStatusCodeFault`.

## Design

### Approach

Build a fluent, plain-Rust API on top of `MockTransactionHandler` as the primary test pattern. The new API lives in `native_exec` (where the FFI interaction happens) and is re-exported by the SDK crate. No new wrapper types around `TestEnvironment` — the ergonomic methods are added directly to it.

### Entry Point: `TestEnvironment`

New ergonomic methods added directly to `TestEnvironment` / `TestEnvironmentHandle`:

```rust
let env = TestEnvironment::no_cost();

// Contract lifecycle
let contract = env.add_mock_contract(|c| {
    c.method(entrypoint_id, || { /* handler */ });
});
let hash = env.register_contract(&contract);
let addr = env.deploy(&hash, &nonce);

// Convenience: register + deploy in one step using TestRegister trait
let addr = env.deploy_mock::<MyContractImpl>();

// Environment management
env.grant_storage_tokens(&addr, 1_000_000_000);
env.advance_block();
```

### Transaction API

Two modes: auto-commit (common) and manual (advanced).

**Auto-commit — the common case:**

```rust
let result = env.transact(|tx| {
    tx.call(&addr)
      .run(|| {
          // Contract code executes here, inside a frame.
          // Syscalls work, caller address is set, etc.
          let bal = get_balance();
          assert_eq!(bal, 100);
      })
});
result.assert_success();
```

`transact()` creates a `MockTransactionHandler`, gives the closure a `Transaction` handle, and auto-commits when the closure returns.

**Manual commit — when you need control over the commit step:**

```rust
let result = env.transact_manual(|tx| {
    let frame_result = tx.call(&addr).run(|| { /* ... */ });

    // Inspect frame result before deciding to commit
    if frame_result.is_success() {
        tx.commit()
    } else {
        tx.abort()
    }
});
```

### Frame Builder

Replaces `open_root_frame()` chain. Root frames do not take invoke flags.

**Root frame (via Transaction):**

```rust
tx.call(&addr)                          // sets contract address
  .with_calldata(data)                  // optional
  .run(|| { /* single-shot */ })        // execute, close frame, return FrameResult
```

**Child frame (via parent frame, advanced):**

```rust
frame.child_call(&other_addr)
    .with_flags(flags)                  // invoke flags (only on child frames)
    .with_calldata(data)                // optional
    .run(|| { /* ... */ })
```

**Continuable frames (advanced, for interleaving/nesting):**

```rust
tx.call(&addr)
  .run_continuable(|frame| {
      let child_result = frame.child_call(&other_addr)
          .run(|| { /* child contract code */ });

      FrameAction::Continue  // keep this frame open
  })
  .run_continuable(|frame| {
      FrameAction::Close
  })
  .finish()                             // extracts FrameResult
```

`FrameAction::Continue` / `FrameAction::Close` replaces the bool return.

### Result Types

Two result types reflecting the execution model:

**`FrameResult`** — returned from any frame (root or child). Wraps `InvocationFrameOutput` + optional `InvocationFrameTrace`.

```rust
impl FrameResult {
    // Status checks
    fn is_success(&self) -> bool;
    fn is_abort(&self) -> bool;
    fn is_fault(&self) -> bool;

    // Assertion methods (panic with descriptive messages on failure)
    fn assert_success(&self);
    fn assert_abort(&self);
    fn assert_fault(&self);

    // Decode abort buffer as a typed error
    fn assert_abort_with<E: Exn>(&self, check: impl FnOnce(&E));

    // Access return data
    fn return_buffer(&self) -> &[u8];   // panics if not success

    // Raw access
    fn output(&self) -> &InvocationFrameOutput;
    fn trace(&self) -> Option<&InvocationFrameTrace>;

    // Per-frame events (TODO: requires adding events to InvocationFrameTrace in hogd)
    fn events(&self) -> &[TxEmittedEvent] {
        todo!("per-frame event access not yet supported — \
               use TransactResult.events() for transaction-level events")
    }
}
```

**`TransactResult`** — returned from `env.transact()`. Wraps a `FrameResult` (from the root frame) plus `ExecResult` and event log. The `ExecResult` covers the 2-phase storage reservation commit, which is a separate step from frame execution.

```rust
impl TransactResult {
    // Delegates to inner FrameResult
    fn assert_success(&self);           // root frame returned + ExecResult OK
    fn assert_abort(&self);
    fn assert_fault(&self);
    fn assert_abort_with<E: Exn>(&self, check: impl FnOnce(&E));

    // DB commit step (ExecResult) — only on TransactResult
    fn assert_commit_failed(&self);
    fn exec_result(&self) -> &ExecResult;

    // Access the root frame result directly
    fn frame(&self) -> &FrameResult;

    // Transaction-level events
    fn events(&self) -> &[TxEmittedEvent];
    fn assert_event(&self, index: usize, check: impl FnOnce(&TxEmittedEvent));
    fn assert_event_data<E: FromEmittedEvent>(&self, index: usize, check: impl FnOnce(&E));
    fn assert_emitted<E: FromEmittedEvent>(&self, predicate: impl Fn(&E) -> bool);
}
```

Note: transaction-level event access requires wiring `get_event_logs()` to `MockTransactionHandler` (the FFI call already exists on the underlying syscall handler). Per-frame events require adding event data to `InvocationFrameTrace` in hogd — left as a TODO/panic for now.

### Event Assertion Examples

```rust
let result = env.transact(|tx| {
    tx.call(&addr).run(|| {
        transfer(&to, 100);
    })
});

// Check event count
assert_eq!(result.events().len(), 1);

// Check raw event by index
result.assert_event(0, |event| {
    assert_eq!(event.num_topics, 2);
});

// Decode and check typed event
result.assert_event_data::<TransferEvent>(0, |e| {
    assert_eq!(e.amount, 100);
    assert_eq!(e.to, to_addr);
});

// Check that any event matches a predicate
result.assert_emitted::<TransferEvent>(|e| e.amount == 100);
```

### Expect-Revert Examples

```rust
// Assert a call aborts
let result = env.transact(|tx| {
    tx.call(&addr).run(|| {
        transfer(&to, 999_999); // insufficient balance
    })
});
result.assert_abort();

// Assert abort with specific error type
result.assert_abort_with::<TokenError>(|e| {
    assert_eq!(e.code, ErrorCode::InsufficientBalance);
});

// Assert frame faulted (panic/unrecoverable)
result.assert_fault();

// Assert the frame succeeded but DB commit failed
result.assert_commit_failed();
```

### Multi-Transaction Scenario Example

```rust
let env = TestEnvironment::no_cost();
let addr = env.deploy_mock::<MyToken>();
env.grant_storage_tokens(&addr, 1_000_000_000);

// Transaction 1: mint tokens
let result = env.transact(|tx| {
    tx.call(&addr).run(|| {
        mint(&owner, 1000);
    })
});
result.assert_success();

env.advance_block();

// Transaction 2: transfer
let result = env.transact(|tx| {
    tx.call(&addr).run(|| {
        transfer(&recipient, 100);
    })
});
result.assert_success();
result.assert_emitted::<TransferEvent>(|e| e.amount == 100);

// Transaction 3: over-transfer should fail
let result = env.transact(|tx| {
    tx.call(&addr).run(|| {
        transfer(&recipient, 999_999);
    })
});
result.assert_abort();
```

### Nested Frames / Multi-Address Example (Advanced)

```rust
let result = env.transact(|tx| {
    tx.call(&contract_a)
      .run_continuable(|frame| {
          // Call contract B from within contract A's frame
          let child = frame.child_call(&contract_b)
              .with_flags(invoke_flags)
              .run(|| {
                  // Executes as contract B
              });
          child.assert_success();
          FrameAction::Close
      })
      .finish()
});
```

## Where the Code Lives

- **New types and methods** — `native_exec` crate (alongside existing `MockTransactionHandler`, `TestEnvironment`)
  - `FrameResult`, `TransactResult`, `FrameAction` — new types
  - `transact()`, `transact_manual()` — new methods on `TestEnvironment`
  - `call()`, `child_call()`, `run()`, `run_continuable()` — new fluent API types
- **SDK re-exports** — the SDK crate re-exports the new API so tests can `use sdk::testing::*` or similar
- **`MockTransactionHandler`** — stays as the underlying engine; the new API wraps it
- **`TransactionHandler`** — stays unchanged for `SignedTransaction` execution; future unification possible but not prioritized

## Out of Scope

- **Gas metering** — these tests run as native code. Gas-metered testing against WASM builds is a future feature.
- **State snapshots & rollback** — useful but not high priority for this iteration.
- **Deprecation/removal of old APIs** — the old macros and helpers in `src/contract_test.rs` and `src/dev/contract_test.rs` continue to work. Deprecation and migration happen later at the author's discretion.
- **Unifying `MockTransactionHandler` and `TransactionHandler`** — they share FFI calls under the hood, but unification is not prioritized. The new ergonomic API builds on `MockTransactionHandler`.
- **`try_invoke` flag semantics on child frames** — child frames with the `try_invoke` flag trigger early 2-phase reservation. The result type for this case may need extension later.
