# `bigint-bench`: per-operation rv32 codesize and gas for 256-bit integers

Date: 2026-08-12
Status: implemented

## Problem

`crates/u256` landed on the strength of measurements taken by hand. Its design
doc's own closing note says so:

> Follow-up: the code-size rule is currently verified by hand. Turning it into a
> standing assertion in the rv32 metrics CI is worth doing, but it needs a
> cross-compiled fixture and did not belong in this change.

Two questions have no repeatable answer today:

1. **Codesize.** What does each 256-bit operation cost in rv32 `.text`, and how
   does `sdk::U256` compare against Parity's `uint` operation by operation? The
   numbers quoted in the U256 design doc (`uint`'s `Div`/`Rem` at 11,300 bytes,
   its `integer_sqrt` at 12,732) came from reading one contract's map file.
2. **Gas.** Nothing has ever measured the *runtime* side. The whole premise —
   that `[u32; 8]` limbs avoid `compiler_builtins` 128-bit libcalls on rv32 — is
   a claim about executed instructions, and the existing gas metric
   (`scripts/metrics/sdk_perf_gas.sh`) reports one number per wasm bottle test,
   which cannot attribute anything to an arithmetic operation.

The target is rv32, not wasm: that is the artifact format with the 64 KiB code
cap the U256 design is written against, and the VM whose gas schedule charges
per instruction.

## Design

### One build per (implementation, operation)

Both numbers are *marginal linkage* numbers, and neither survives being measured
in a binary that carries more than one operation.

- LLVM shares code across operations. `div` and `rem` are one routine; a
  Newton-iteration `integer_sqrt` inlines a whole division. Per-symbol
  attribution inside a combined binary double-counts the shared part and hides
  where it came from.
- The question a contract author actually asks is "what does reaching for this
  cost me" — which is by definition the difference between a binary that calls
  it and one that does not. That is what the U256 design doc measured when it
  reported "reaching for the general `U256 / U256` adds 2,044 bytes over a build
  that only calls `div_rem_small`", and it is the only shape that answers the
  code-size rule the doc left unverified.
- Gas has the same problem in a milder form: an artifact linking every operation
  has different dispatch and a different code layout than the one a real
  contract deploys.

So `example-contracts/bigint-bench-contract` is built once per (backend,
operation) pair, each with exactly one `op-*` cargo feature. A const assertion
in `kernel.rs` fails the build if zero or two are enabled — the failure mode
otherwise is a plausible-looking number, not an error.

Cost: 34 rv32 builds with LTO, a few minutes. That is why this is a
`scripts/dev.sh bigint-bench` command and a metrics script rather than part of
`dev.sh ci`.

### The `none` calibration build

Every reported figure is a difference against a build of the same loop with an
identity operation. That build carries the entrypoint dispatch, the `u128`
calldata decode, the operand round trips through `black_box`, the loop counter
and the result digest — and no arithmetic. Subtracting it removes the harness
from both metrics at once, which is what makes two implementations with
different calling conventions comparable.

It is also why the harness may be as expensive as it likes. `black_box` on both
operands forces a 32-byte memory round trip per iteration; that cost is
identical in the calibration build and cancels exactly.

### Preventing the measurement from being optimized away

A benchmark of pure functions over compile-time-known operands measures nothing.
Three things keep the work alive:

- Operands arrive as calldata (`u128` halves), so their values are unknown at
  compile time.
- `kernel::drive` wraps both inputs and the result of every iteration in
  `core::hint::black_box`, so the body cannot be hoisted or CSE'd across
  iterations.
- The contract returns a digest folding every limb of the result, and
  `tests/differential.rs` asserts the digest changes when any single bit of the
  input does. Without that, LLVM could drop the arithmetic feeding the bits the
  digest ignored.

This is not theoretical. The `bytes` operation (`from_be_bytes ∘ to_be_bytes`)
initially reported a marginal `.text` of exactly **zero** in both backends: the
pair is the identity, `drive`'s barriers sit outside the call, and LLVM folded it
away. It needs a barrier *between* the two halves, which `ops::bytes` now has.

### Operand shape across the ABI

`run` takes four `u128` halves, not a 256-bit contract type. Passing `sdk::U256`
would put its XDR decode in the calldata path of the *`uint`* builds too, so a
build nominally measuring Parity's implementation would link `groundhog-u256`
code. Primitives keep the two artifacts disjoint.

### The backend trait, and what "the same operation" means

`backend.rs` defines one trait; each implementation gets one impl. Kernels are
generic over it, so a build instantiates it exactly once and LTO sees the shape
it would see had the contract called the implementation directly. Generic
kernels also mean an unused operation emits no code at all, rather than relying
on `--gc-sections` to remove it.

Each method maps to the API a contract author would reach for **at that
implementation's native limb width**, not to the closest literal spelling.
`wrapping_mul_small` is the case that matters: `sdk::U256` takes a `u32` scalar
(`overflowing_mul_u32`), `uint` takes a `u64` one (`overflowing_mul_u64`),
because those are the two libraries' single-limb multiplies. Comparing U256's
u32 path against a full 256x256 `uint` multiply would measure the wrong thing.

A mapping mistake here is the failure mode that matters, because it shows up as
a performance win rather than an error. `tests/differential.rs` is the guard: it
asserts both backends produce bit-identical results for every operation in the
sweep, over boundary values plus a 60,000-case deterministic sweep. It runs on
the host, needs neither rv32 nor the node, and is `scripts/dev.sh
bigint-bench-test`.

Operations whose operands the type does not supply directly derive them from
`a`/`b` by the same expression in both backends, so the derivation cancels:
divisors are forced odd with `| 1` (both implementations panic on zero), shift
distances are reduced `% 256` (where `wrapping_shl`'s masking and `uint`'s
shift-off-the-top agree), and `mul_div_u128`'s divisor is `b | 1`, which keeps
the quotient inside a `u128` for every input — a precondition, since
`sdk::mul_div_u128` panics when it overflows.

## Layout

| Path | Role |
| --- | --- |
| `crates/bigint-bench-if` | The `BigIntBench` trait and its client. Shared by the guest contract and the host runner, the way `bottle-test-if` is. |
| `example-contracts/bigint-bench-contract` | The contract: backend trait, both impls, the operations, the measured loop, and the rv32 bin wrapper. Carries a `disabled` marker. |
| `crates/bigint-bench` | Host runner: deploys each artifact, measures gas, joins with codesize, renders the diff. |
| `scripts/bigint-bench.sh` | Build driver: the per-variant rv32 builds and the `readelf` codesize scrape. |
| `scripts/metrics/sdk_bigint_bench_rv32.sh` | Metric-history wrapper. |

The contract package sits in `example-contracts/` because that workspace holds
the canonical `[target.riscv32im-unknown-none-elf]` rustflags; the root
workspace deliberately has none (duplicating them redefines the linker script's
memory regions). It is named without the `-rv32` suffix, and carries a
`disabled` marker, so that neither the wasm build loop, the wasm bottle-test
sweep, `eg_test_all`, nor `sdk_codesizes_rv32.sh`'s append-only metric picks up
34 build variants of it. `dup_entrypoint_neg` uses the same two devices.

## Measured

rv32 `.text` bytes each operation adds over its backend's `none` build, at
`opt-level = 3`, LTO, `codegen-units = 1` (the contract release profile):

| operation | `sdk::U256` | `uint` | Δ |
| --- | --- | --- | --- |
| add | 224 | 344 | +120 (+54%) |
| sub | 276 | 376 | +100 (+36%) |
| mul | 1284 | 1140 | −144 (−11%) |
| mul_small | 196 | 232 | +36 (+18%) |
| widening_mul | 596 | 576 | −20 (−3%) |
| div | 3828 | 11624 | +7796 (+204%) |
| div_small | 680 | 5828 | +5148 (+757%) |
| rem | 3956 | 11540 | +7584 (+192%) |
| mul_div_u128 | 3988 | 11280 | +7292 (+183%) |
| shl | 428 | 700 | +272 (+64%) |
| shr | 488 | 1076 | +588 (+120%) |
| bitwise | 152 | 152 | 0 |
| cmp | 76 | 124 | +48 (+63%) |
| sqrt | 1820 | 13636 | +11816 (+649%) |
| bytes | 748 | 748 | 0 |
| bit_len | 220 | 332 | +112 (+51%) |

Calibration builds: `u256.none` 3292 bytes `.text`, `uint.none` 3296 — within
four bytes, so the two harnesses are the same size and the comparison is not
being carried by the scaffolding.

The kept ELFs are unstripped (the driver passes
`--config profile.release.strip=false`, which changes no size it reports), so any
surprising row can be attributed down to symbols without rebuilding:

```
llvm-readelf -sW target/bigint-bench/uint.div_small.elf \
  | awk '$4=="FUNC" {print $3, $8}' | sort -rn | head
```

This reproduces the U256 design doc's hand measurements independently.
`uint`'s divide lands at 11,624 bytes against the 11,300 quoted there, and its
`integer_sqrt` at 13,636 against 12,732 — both within what a different contract
and optimization level explains. Three results the doc predicted but never
measured across implementations:

- **`div_small` is where the design pays off most, proportionally**, and it is
  the libcall thesis landing exactly as written. 680 bytes against 5,828 — a
  factor of 8.6. The symbol table says where the difference goes:

  | build | symbols |
  | --- | --- |
  | `u256.div_small` | `drive::<GroundhogU256, div_small>` 968 B. No compiler-builtins divide at all. |
  | `uint.div_small` | `drive::<PU256, div_small>` 4,964 B, plus `compiler_builtins::int::specialized_div_rem::u64_div_rem` 1,100 B and `__udivdi3` 52 B. |
  | `u256.none` | `drive::<GroundhogU256, none>` 288 B — the harness floor both marginals subtract. |

  `uint`'s `div_mod_small` divides limb by limb through
  `div_mod_word(hi, lo, y) = ((hi as u128) << 64 | lo) / y as u128`. On a 32-bit
  target there is no 128-by-64 divide instruction to lower onto, so each of the
  four iterations becomes a compiler-builtins call. That is precisely the bug
  the U256 design doc reports finding in its *own* first draft one width down —
  a `((rem as u64) << 32 | limb) / d` that pulled in `__udivdi3` — and fixed by
  replacing it with `div_rem_2by1` (Hacker's Delight `divlu`, base 2^16, every
  divide a native 32-bit `DIVU`). `uint` still has the u64 spelling of it, and
  this is what it costs.
- **`sqrt` is the largest absolute win**, 1,820 against 13,636. Binary restoring
  versus Newton iteration, whose every step is a full 256-bit divide that LLVM
  inlines into the loop.
- **The full 256×256 multiply is `uint`'s to win, slightly** — 1,140 against
  1,284, and `widening_mul` likewise. Four 64-bit limbs need 16 partial products;
  eight 32-bit limbs need 64. The libcall tax the design doc is about applies to
  the *intermediates*, and on this operation `uint` spends its 64-bit limbs well.
  Whether the extra 144 bytes buys back the instruction count is a gas question
  this harness answers, not a codesize one.

### Gas: implemented, not yet run here

The gas half is written and compiles; it has not been executed in the
environment this was developed in. `sdk`'s mock node stands up through the hogd
test FFI, whose x23 RPC layer binds `AF_INET6` unconditionally, and this
container's kernel has no IPv6 (`/proc/sys/net/ipv6` absent) — every
`TestEnvironment::new` fails with `create_db: 5` here, including the existing
`example-contracts` tests. Nothing about that is specific to this change; CI
runners have IPv6 and the wasm bottle tests run there today.

So the numbers above are measured and the gas column is not. `-s`
(`--no-gas`) exists for that case: it reports the codesize half without starting
the node, and is what produced the table above. Dropping `-s` on a host with a
working mock node fills in the gas and Δgas columns.

## Extending it

**A new operation:** add an `op-*` feature to the contract's `Cargo.toml`, a
generic function to `ops.rs`, a row to the `kernels!` table in `kernel.rs`, a
name to `OPS_ALL` in the driver, a row to `agreement_tests!`, and a name to
`DECLARED` in the runner's `operations()` (which only controls display order).
The const assertion catches a missed `kernels!` row; cargo catches a missed
feature; nothing catches a missed differential test, which is the one to be
careful about.

**A new implementation** (`ruint`, `crypto-bigint`, a hand-rolled candidate): add
an optional dependency and a `backend-*` feature, a module to `backend.rs`
implementing `Backend`, an arm to `backend_feature()` in the driver, and a name
to `BACKENDS_ALL`. Then extend `tests/differential.rs` to cover it — the
`assert_agrees` helper is written against two concrete types and would want
generalizing to a set. Keep the dependency optional: a build that links two
implementations measures neither.

## Alternatives considered

**One contract per backend with an entrypoint per operation, and per-symbol
codesize from the ELF symbol table.** Two builds instead of 34, and gas
measured per entrypoint in one deployment. Rejected for codesize: shared
helpers make per-symbol attribution wrong in a way that is invisible in the
output (`div` and `rem` would each be charged the routine, or one of them
charged nothing depending on which won the symbol). It would also have hidden
the `div_small` result entirely, which is a *linkage* fact and has no symbol.

**Feature-gating the operation but sharing one binary across backends.**
Rejected: a build linking both implementations measures neither, and the
`uint` builds would carry `sdk::U256`'s calldata decode.

**Reusing the `BottleTest` trait (`setup`/`run`).** It has no way to say which
operation to run or how many iterations, and its two-contract structure exists
to measure cross-contract calls — overhead this harness wants absent. The
benchmark keeps the bottle-test *shape* (a contract in the mock node, gas read
across a client call from a host frame) and its own interface.

**Criterion-style host benchmarking.** Measures the wrong machine. Gas is
charged per rv32 instruction by the artifact's own gas schedule; host wall-clock
on x86 says nothing about it.
