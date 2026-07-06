# Native-exec per-frame allocation instrumentation

**Date:** 2026-07-06
**Status:** approved (interactive brainstorm with Geoff)

## Motivation

rv32 contracts run against a bump allocator with a hard heap bound
(`rv32-runtime/src/allocator.rs`: `dealloc` is a no-op, `realloc` uses the
default alloc+copy+dealloc path). When contracts run natively in
`groundhog-native-exec` tests, we want per-invocation-frame allocation
stats as a proxy for guest heap consumption — to catch regressions and
estimate whether a contract fits its heap budget.

The important frames are the **cross-call frames**: when contract code
under test does `client.foo()`, the call goes
`contract_call` → `raw_Invoke` syscall → hogd → `trampoline`
(`test_environment.rs`) → the `invoke_wrapper`-wrapped method registered
via `MockContract::register_method`. The callee's entire native execution
is that closure call, on the same OS thread as the caller. Secondarily,
the harness-driven frames (`FrameBuilder::run` closures via
`MockInvocationFrame::execute`) also hold contract code under test.

## Design

No external dependencies. One new module `native-exec/src/alloc_stats.rs`.

### Counting allocator

`CountingAllocator` wraps `std::alloc::System`. On `alloc` it records
`(size, align)` into thread-local counters, then forwards. `dealloc`
forwards without recording. `realloc`/`alloc_zeroed` are **not**
overridden: the default impls route through `alloc`, charging reallocs
as fresh allocations — exactly what the rv32 bump allocator does.

Thread-locals are const-initialized `Cell<u64>`s (no lazy init — lazy TLS
init can allocate and recurse; no destructors, so no dtor registration).
Thread-locals also isolate parallel `cargo test` threads.

Counters (all monotonic):
- `allocs`: number of allocation calls
- `bytes_total`: cumulative bytes requested
- `sim_cursor`: simulated rv32 bump cursor — replays the guest
  allocator's arithmetic (`aligned = round_up(cursor, align); cursor =
  aligned + size`) so per-frame deltas include alignment padding the
  guest would pay.

Installation: `#[global_allocator]` must live in the final binary, so
export `groundhog_native_exec::install_counting_allocator!();` for test
binaries to invoke once at top level. Without it everything compiles and
runs; stats read zero.

### Per-frame measurement with exclusive accounting

In the real system each frame is its own guest instance with a fresh
heap, so per-frame stats must be **exclusive**: a caller is not charged
for its callees. The thread-local state is one measurement context —
the counters (allocs, bytes, bump cursor) of the *innermost open
window*, plus a window-depth counter and the pause flag:

```
FrameMeasureScope::open():  saved = take(counters, zeroing them); depth++
FrameMeasureScope::close(): exclusive = counters; counters = saved; depth--
                            return exclusive
```

The window "stack" lives implicitly in the nested scope values on the
Rust call stack. A frame never observes its callees' allocations at
all (no subtraction algebra), and `heap_used` is an exact replay of the
frame's own allocation sequence on a fresh guest heap — zeroing the
cursor at open and restoring the parent's at close reproduces exactly
the per-instance heap of the real target. (An earlier revision kept
global monotonic counters and subtracted children via a `CHILD_CHARGED`
accumulator; review found the save/zero/restore context both simpler
and more faithful, and it collapses the per-allocation cost to one TLS
access.)

### Hook points

1. **`invoke_wrapper`** (cross-call frames — the primary target): the
   existing wrap-a-contract-fn-for-FFI chokepoint takes the
   `(Contract, entrypoint)` label, opens a scope around its
   `catch_unwind` (so abort paths measure too), and pushes a
   `FrameAllocRecord { contract, entrypoint, stats }` into a
   thread-local log. The log is internal: each enclosing frame window
   claims the records pushed during it (an index snapshot at window
   start + `split_off` at close — inner windows claim their tail first,
   so nesting works without a tree structure), and surfaces them as
   `FrameResult::cross_call_alloc_records()`, in completion order,
   right next to the frame's trace and events. There is no global
   drain/reset API for tests to manage. A record pushed while no window
   is open (a mock dispatched outside the mock-frame harness, e.g.
   `TransactionHandler::root_invoke`, or a wrapped closure invoked
   directly) has no owning frame and is dropped rather than stranded.

2. **`MockInvocationFrame::execute`** (harness-driven frames): open a
   scope around the `catch_unwind`, accumulate exclusive stats onto the
   frame across multiple `execute` calls (`FrameAction::Continue`), and
   surface them as `FrameResult::alloc_stats()`. `FrameResult::new`
   keeps its signature (stats default to zero) to avoid churn in
   existing callers; the frame machinery attaches measurements via a
   `with_alloc_measurements` constructor helper.

   Embedding into `InvocationFrameTrace` itself was considered but that
   type is generated in the registry `groundhog-trace` crate (it
   arrives serialized from hogd), so the records live beside `trace()`
   on `FrameResult` instead; correlating records to trace nodes by
   completion order is possible later if wanted.

#### Excluding harness bookkeeping

Harness allocations must not be charged to frames (found in review:
record-`Vec` growth and child-frame teardown were leaking into the
enclosing frame's numbers). A thread-local `PAUSED` flag gates
`record_alloc`; `PauseGuard` (RAII, restores on drop, unwind-safe)
marks harness regions: `perform_mocked_syscall` (syscalls are host-side
work — they never touch the guest heap), frame open
(`MockInvocationFrameConfig::build`), frame teardown after the
measurement window closes, `FrameResult` construction, and record
bookkeeping. `FrameMeasureScope::open` clears the flag (restoring it on
close), so contract code re-entered *through* a paused region — the
`Invoke` syscall trampolining into a mock method — still measures. A
frame whose contract code allocates nothing reports exactly zero, even
with child frames and cross-calls (asserted by test).

## Known approximations (documented on the API)

- Native sizes ≠ rv32 sizes (8-byte pointers/`usize` vs 4). Good for
  relative comparisons and regression bounds, not exact budget math.
- The measured window includes allocations by harness code running *as*
  the frame's closure (e.g. `FrameBuilder::run`'s XDR-encode `Vec`; on
  the guest the same encode uses the no-alloc scratch buffer,
  `contract_op.rs`).
- A `panic!` raised directly in contract code allocates its payload box
  natively inside the measured window; panics raised by syscalls are
  excluded along with the syscall.
- A frame spanning several `execute` steps (`FrameAction::Continue`)
  replays each step from a fresh cursor, so padding at step boundaries
  is not charged; the real guest keeps one heap across the whole frame.

## Testing

- Unit tests (pure, no allocator installed): `record_alloc` math
  including alignment padding; nested/sibling scope exclusivity; record
  drain semantics.
- Integration tests (`tests/alloc_stats.rs`, macro installed): root
  frame sees a known `Vec::with_capacity` allocation; explicit child
  frames and trampoline cross-calls are charged to the callee record,
  not the caller; assertions use `>=` bounds to tolerate harness noise.
