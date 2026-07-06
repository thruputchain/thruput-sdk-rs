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
for its callees. Frames nest strictly on one thread, so a thread-local
`CHILD_CHARGED` accumulator suffices:

```
FrameMeasureScope::open():  start = totals(); child0 = CHILD_CHARGED
FrameMeasureScope::close(): inclusive = totals() - start
                            child     = CHILD_CHARGED - child0
                            CHILD_CHARGED = child0 + inclusive   // absorb grandchildren
                            return inclusive - child             // exclusive
```

Setting (not adding) `CHILD_CHARGED = child0 + inclusive` at close makes
a parent see exactly the sum of its *direct* children's inclusive
deltas, at any nesting depth.

### Hook points

1. **`MockContract::register_method`** (cross-call frames — the primary
   target): wrap the registered method so each trampoline invocation
   opens a scope, runs the `invoke_wrapper`ed closure (measurement
   closes after its internal `catch_unwind`, so abort paths measure
   too), and pushes a `FrameAllocRecord { mock_contract_index,
   entrypoint, stats }` into a thread-local log. Tests read it via
   `alloc_stats::drain_frame_records()`.

2. **`MockInvocationFrame::execute`** (harness-driven frames): open a
   scope around the `catch_unwind`, accumulate exclusive stats onto the
   frame across multiple `execute` calls (`FrameAction::Continue`), and
   surface them as `FrameResult::alloc_stats()`. `FrameResult::new`
   keeps its signature (stats default to zero) to avoid churn in
   existing callers; the frame machinery attaches stats via a
   `with_alloc_stats` constructor helper.

### Known approximations (documented on the API)

- Native sizes ≠ rv32 sizes (8-byte pointers/`usize` vs 4). Good for
  relative comparisons and regression bounds, not exact budget math.
- The measured `execute` window includes small harness allocations
  (e.g. `FrameBuilder::run`'s XDR-encode `Vec`; on the guest the same
  encode uses the no-alloc scratch buffer, `contract_op.rs`).
- Panic unwinding (abort paths, `raw_Return`'s control-flow unwind)
  allocates its payload box natively; the guest equivalent does not.
- `sim_cursor` is a single global replay, so a frame's first-allocation
  padding depends on prior history (bounded by max align, ~16 bytes).

## Testing

- Unit tests (pure, no allocator installed): `record_alloc` math
  including alignment padding; nested/sibling scope exclusivity; record
  drain semantics.
- Integration tests (`tests/alloc_stats.rs`, macro installed): root
  frame sees a known `Vec::with_capacity` allocation; explicit child
  frames and trampoline cross-calls are charged to the callee record,
  not the caller; assertions use `>=` bounds to tolerate harness noise.
