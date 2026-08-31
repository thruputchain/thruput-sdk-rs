# `I256` and the OpenZeppelin math surface for `groundhog-u256`

Date: 2026-08-31
Status: implemented

## Problem

`crates/u256` shipped [`U256`](2026-08-11-u256-crate-design.md) with `I256`
listed as a non-goal, for a stated reason: "nothing in the swap suite uses it
today, so there is no call site to shape the API against — in particular
floor-vs-truncate division semantics, and which of `abs`/`signum`/`checked_neg`
matter. Deferred until a consumer exists."

A consumer now exists. A client is porting contracts that use signed 256-bit
arithmetic, and expects the utility surface that comes with it on Ethereum:
`@openzeppelin/contracts/utils/math/Math.sol` and `SignedMath.sol`. Those two
libraries are the vocabulary an Ethereum contract author writes in. A port that
has to hand-roll `mulDiv` — the one function in `Math.sol` that is genuinely
hard, because it needs a 512-bit intermediate — is a port that will get it
wrong.

The same design note also deferred "a general `mul_div(U256, U256, U256)`,
[because] exactness requires a 512-bit intermediate", shipping only the
`u128`-in/`u128`-out `mul_div_u128`. That is precisely `Math.mulDiv`, so the
deferral lapses with the same request.

## Goals

- `I256`: a signed 256-bit integer with the semantics of Rust's primitives,
  which on every point that matters are also Solidity's `int256`.
- The `Math.sol` surface on `U256` and the `SignedMath.sol` surface on `I256`,
  under names an Ethereum author will recognise.
- Full-precision `mul_div` and `mul_shr`: an exact 512-bit intermediate, with
  the overflow reported rather than truncated.
- No second copy of the delicate code. One Knuth D, one two's complement
  representation.

## Non-goals

- `U512` as a public type. The 512-bit value exists only as the intermediate
  of `mul_div`/`mul_shr`/`mul_mod` and as the `(low, high)` pair
  `widening_mul_u256` returns. Exposing a whole third numeric type to hold what
  three functions consume internally is a much larger surface than the request.
- `SafeCast.sol`. Rust already has `TryFrom`, and this change wires it up for
  every width in both directions.
- `Math.modExp`'s arbitrary-length `bytes` variant — and, in a sense, `modExp`
  itself. Worth being precise about what OpenZeppelin actually ships here,
  because it changes what "parity" means.

  `Math.modExp` is not an implementation. Its body is a `staticcall` to the
  EIP-198 precompile at address `0x05`; the bignum runs as native client code,
  which is why the function is `view` rather than `pure` and why the `bytes`
  variant costs Solidity nothing beyond a different memory layout in the same
  call.

  Groundhog's syscall table is the real analogue of the EVM's precompile slots,
  and it carries `kSHA256` and `kVerifySignature` — EVM `0x01` and `0x02` — but
  nothing for modular exponentiation. (`crates/sdk/src/precompiles.rs` is an
  unrelated thing: well-known *contract addresses* like the native token, not
  computational precompiles.) So the fixed-width `mod_exp` here is a real
  square-and-multiply loop running inside the contract, which is why it carries
  a cost warning no other function in the crate needs.

  The `bytes` variant on top of that would need an arbitrary-precision integer:
  a different numeric type with its own multiply, divide and allocation story,
  in a crate whose reason for existing is that one *fixed-width* divide is 11 KB
  of rv32 text. And every value this SDK moves is at most 256 bits, so a `bytes`
  entry point over ≤32-byte inputs would be `mod_exp` behind a decode — a
  wrapper with no capability. Out of scope, not deferred.

  **If modular exponentiation becomes load-bearing** — RSA verification,
  anything with a pairing behind it — the faithful answer is a host syscall
  beside `kSHA256`, not a better loop in this crate. That is a protocol change
  in hogd rather than an SDK one, and it is the shape of the request to make.
- Constant-time guarantees. Unchanged from the original design: this is not a
  cryptographic bignum. `mod_exp` and `inv_mod` branch on their operands.

## Design

### `I256` is `U256`'s bits, read signed

```rust
pub struct I256 { limbs: [u32; 8] }   // two's complement, LS-first
```

The representation is deliberately identical to `U256`'s, not merely similar.
In two's complement, addition, subtraction, multiplication and every bitwise
operation produce the same 256 bits whichever way the operands are read, so
those route straight through `cast_unsigned` to the existing `U256` code and
only the *overflow test* is signed. What is genuinely separate is small:
comparison (flip the sign bit, then compare unsigned), division (magnitudes
through `U256::div_rem`, sign applied after), the arithmetic right shift, and
formatting.

`cast_signed`/`cast_unsigned` are the reinterpreting conversions, named after
`i128::cast_unsigned`. `TryFrom` is the checked one in both directions.

Semantics follow the primitives:

- Division truncates toward zero and the remainder takes the sign of the
  dividend (`-7 / 2 == -3`, `-7 % 2 == -1`) — which is also Solidity's rule, so
  the deferred "floor-vs-truncate" question answers itself now that the
  consumer is a Solidity port. `div_euclid`/`rem_euclid` are there for the
  floor convention.
- `>>` is arithmetic.
- `MIN.abs()` and `-MIN` overflow; they panic under `overflow-checks` and wrap
  without it, exactly as the `U256` operators do. `unsigned_abs` is the total
  version, and returns a `U256` — the same shape `SignedMath.abs` has.
- `MIN / -1` and `MIN % -1` overflow too, and **panic unconditionally**,
  release profile included. The first cut gated them on `overflow-checks` for
  symmetry with the rest of the type, and that was wrong: division overflow is
  the one arithmetic overflow the primitives themselves leave ungated, because
  on real hardware the alternative is a trap rather than a wrapped value
  (verified — `i128::MIN / -1` panics under `-C overflow-checks=off`). Gating
  it would have meant a contract silently computing `MIN` where the primitive
  aborts, on an input an attacker can often reach, and no host test could see
  it because the harness builds with checks on.

**Wire format.** 32 big-endian two's complement bytes: byte for byte what a
`U256` with those bits encodes, and what Ethereum's `int256` uses. The spec
type is a named struct `I256` with one `Array(U32, 8)` field, so `U256` and
`I256` differ on the wire in exactly one byte — the first character of the type
name. `tests/spec_abi.rs` pins that, including the one-byte claim.

A distinct spec name rather than a shared one is the whole safety property
here: a decoder that confused them would read the same 32 bytes with the wrong
sign.

### One Knuth D body, two widths

`Math.mulDiv` needs a 512-bit dividend. The choice was between a second copy of
Knuth algorithm D specialised to sixteen limbs, and generalising the existing
one to serve both.

**The first answer was wrong, and the benchmark said so.** Generalising looks
free: `div_rem_knuth` takes the dividend as a limb slice with an explicit
length, every loop is bounded by the runtime `nn`/`dn`, so the wider buffers
should cost the narrow caller stack and no instructions. Measured on rv32 with
`scripts/bigint-bench.sh -b u256 -o "div div_small mul_div_u128"`, marginal
over the `none` calibration build:

| | `div` .text | `div` gas | `div_small` | `mul_div_u128` .text | `mul_div_u128` gas |
|---|---|---|---|---|---|
| before this change | 2,640 | 963.05 | 680 / 756.04 | 3,036 | 1735.05 |
| one shared function | 2,576 | **1221.05** | 680 / 756.04 | 2,996 | 1798.05 |
| two instantiations | 2,572 | 972.05 | 680 / 756.04 | 3,012 | 1712.05 |

Sharing buys 64 bytes and costs **+258 gas per division, 27% on these
operands** — the zero-init and copy-out of buffers twice the size the narrow
path can ever fill. Division is the hot path of the contracts this crate exists
for, so that trade is backwards.

**That 27% is a worst case, not a constant**, and it is worth saying why before
anyone quotes it. The overhead is per call, so it is a larger fraction of a
shallow division than a deep one — and the bench's fixed operands make `div`
shallow. Its divisor is 201 bits, seven of eight `u32` limbs, so Knuth D runs
its quotient loop **twice out of a possible seven**; that is also why `div`
(972 gas) sits so close to `div_small` (756), which always does eight 2-by-1
divides. Nor is the overhead purely fixed: the same change cost
`mul_div_u128` only 63 gas through the same `checked_div_rem`, because LLVM
elides more of the oversized zero-init in that call shape. What holds across
every row measured is the sign — specialised is never worse than shared — which
is what the decision rests on.

What shipped instead: the algorithm is written once as a `macro_rules!` body
and instantiated at two dividend widths, each with buffers sized to its own.
One source of truth, two specialised functions. That gets the bytes anyway (68
below the pre-change baseline, since the eight-limb quotient no longer round
trips through a sixteen-limb array) for +0.9% gas, and leaves `mul_div_u128`
both smaller and faster than before the change.

`#[inline]` on the generated function was measured too, on the theory it would
restore the pre-split shape exactly: it costs 156 bytes on `div` to save 2 gas,
and makes `mul_div_u128` both bigger and slower. Not taken.

The cost of specialising is that a contract using both `/` and the
full-precision family links Knuth D twice — and that half is measured too, by
a `mul_div_and_div` op that calls both paths in one kernel so the artifact
carries both instantiations (every other row builds exactly one, which is why
this needed its own op):

| | .text | gas |
|---|---|---|
| `mul_div` alone | 3,992 | 4712.05 |
| `div` alone | 2,572 | 972.05 |
| both in one artifact | 6,348 | 5684.05 |

Adding `/` to a contract that already links `mul_div` costs **2,356 bytes**,
against 2,572 standalone — barely 200 bytes shared, so the two copies are all
but fully duplicated. Sharing would have saved most of that ~2.1 KB and cost
27% of every division's gas. Both halves of the trade are now numbers rather
than one number and one guess. (This document previously put the second copy at
"the 1,892 bytes one copy costs over `div_small`", inferred from the
single-kernel rows. The measurement says 2,356 — the inference was 20% light.)

The original design's code-size rule is untouched: `div_rem_small` is still a
separate function, so a contract that only divides by constants (or formats a
decimal) still links the cheap routine and not Knuth D. The `dn == 1` case of
the 512-bit divide stays on it too, by chaining two calls through a new
`div_rem_small_from`, which seeds the long division with the high half's
remainder instead of zero.

Solidity's `mulDiv` uses Remco Bloemen's trick — make the division exact by
subtracting `mulmod(x, y, d)`, then multiply by the modular inverse of the
denominator mod 2²⁵⁶. That is the right algorithm for the EVM, where a 256-bit
multiply is one opcode and a 512-bit division is not expressible. Here the
division already exists and is measured; the inverse-based version would have
been a second implementation of the same function, plus a Newton iteration, for
no gain.

### The `Math.sol` surface

Everything in `Math.sol` maps, and three members deliberately do not become
functions:

| Solidity | Here |
|---|---|
| `tryAdd`/`trySub`/`tryMul`/`tryDiv`/`tryMod` | `checked_add` and friends — Solidity returns `(bool, uint256)` for want of `Option` |
| `ternary` | `if`. It is a branchless-select trick for the EVM's gas model |
| `clz` | `leading_zeros`, which already answers 256 for zero exactly as `Math.clz` does |
| `add512`, `mul512` | `overflowing_add`, `widening_mul_u256` (halves ordered `(low, high)`, matching `u128::widening_mul` — Solidity returns them the other way round) |
| `mulDiv`, `mulShr` | `mul_div`, `mul_shr`, plus `_rounding` and `checked_` forms |
| `invMod`, `invModPrime`, `modExp` | `inv_mod`, `inv_mod_prime`, `mod_exp` |
| `sqrt`, `log2`, `log10`, `log256` | `integer_sqrt` (the name the crate already shipped) and the log family under their own names, each plus a `_rounding` variant |
| `Rounding`, `unsignedRoundsUp` | `Rounding`, `Rounding::unsigned_rounds_up` |
| `average`, `ceilDiv`, `min`, `max` | same names, all `const fn` |
| `saturatingAdd`/`Sub`/`Mul` | already present since the original crate |

Two additions beyond the port: `mul_mod` and `add_mod`, the EVM primitives that
`Math.sol` uses without re-exporting (there is no opcode to lean on here, so
they have to be functions), and `pow`, which `log10_rounding` needs and which
the primitives have.

`Rounding` also gains `rounds_away_from_zero(negative)`. `unsignedRoundsUp` can
be one function in Solidity because `Floor`/`Trunc` and `Ceil`/`Expand` collapse
for a value that cannot be negative; the signed `mul_div` is where they stop
collapsing.

**`log10` avoids Knuth D.** Solidity's ladder divides by 10⁶⁴, 10³², 10¹⁶ — all
256-bit constants, so all Knuth D. Peeling eight digits at a time by 10⁸ (the
largest power of ten that is both a `u32` and a whole number of digits) keeps
the whole function on `div_rem_small`, at most nine iterations for the widest
input.

**`inv_mod` carries magnitudes, not signed values.** Solidity's version keeps
the Bézout coefficient in an `int256` and notes that it "can overflow".
Consecutive coefficients in extended Euclid always have opposite signs, so the
recurrence on magnitudes is `t[k+1] = t[k-1] + q*t[k]` with an implied
alternating sign, and the standard bound `t <= n / gcd` keeps every intermediate
inside a `U256`. The sign falls out of the parity of the final index.

### `SignedMath.sol`, plus signed `mul_div`

`SignedMath.sol` is five functions: `min`, `max`, `average`, `abs` and
`ternary`. The first four are here (`abs` being `unsigned_abs`, already total
and already returning an unsigned value as Solidity's does); `ternary` is `if`.
`average` rounds toward zero, which is Solidity's documented behaviour and
differs from `U256::average` only in the correction term.

Solidity has no signed `mulDiv`, but a signed 256-bit type without one is not
usable for the case that motivates reaching for it. `I256::mul_div` is the
obvious composition — magnitudes through `U256::mul_div`, sign applied after —
truncating toward zero, the same rule `/` uses, with a `_rounding` variant
keyed off `Rounding::rounds_away_from_zero`. It is documented as a composition
rather than a port, so nobody goes looking for the Solidity original.

`mul_div_parts` returns the sign alongside the quotient rather than letting the
caller re-derive it, because a zero-magnitude quotient loses it — and the
rounding direction of an inexact zero still depends on which side of zero it
came from.

### Where the operators diverge from Solidity, deliberately

The library functions match OpenZeppelin on every degenerate input, traced from
the Solidity source and pinned case by case in `tests/oz_parity.rs` — zero
moduli, `0^0`, `invMod` vs `invModPrime` disagreeing on a zero modulus,
`ceilDiv(0, 0)` reverting rather than answering zero, `mulShr` never rounding
at a zero shift, `clz` against `255 - log2(x)`.

The *operators* follow Rust, and on two points Rust and Solidity disagree:

| | Solidity (checked, default) | Solidity `unchecked` | Rust, and here |
|---|---|---|---|
| `x / 0`, `x % 0` | revert | revert (not disableable) | panic |
| `MIN / -1` | revert | wraps to `MIN` | panic |
| `MIN % -1` | `0` | `0` | **panic** |

`MIN / -1` matches Solidity's default, which is the one that matters:
`unchecked` is opt-in per block there, whereas `overflow-checks` here is a
whole-build setting nobody wrote at the call site. Treating them as equivalent
would mean a port silently computing `MIN` where its original reverted.

`MIN % -1` is the genuine divergence. Solidity's `smod` is a separate opcode
that answers 0, and 0 is the correct remainder; Rust panics because x86
computes quotient and remainder with one trapping instruction. We follow Rust
so that `I256` and the `i128` beside it in an expression behave alike — the
rule the rest of this type's overflow behaviour rests on — and document
`wrapping_rem` as the escape hatch. This was considered and chosen, not
inherited.

## Testing

Three oracles, chosen per property.

- **`i128` and `u128`** are the specification. Every operation whose operands
  and result fit in 128 bits must agree exactly; that is what "behaves like the
  primitives" means, and it is the property a Solidity port depends on. For
  overflow *flags*, where an `i128` overflow is not an `I256` overflow, the
  test shifts both operands up by 128 bits so the two widths coincide.
- **Parity's `uint`** (already a dev-dependency, as the unsigned oracle) pins
  the two's complement claim directly: the bits of a signed add/sub/mul/bitop
  equal the bits of the unsigned one.
- **A 512-bit `uint::construct_uint!`** is the oracle for the full-precision
  family, whose whole point is an intermediate no 256-bit oracle can hold. Both
  divisor paths (single-limb and Knuth D) get their own sweep, and the boundary
  sweep asserts it reached both the fits and overflows answers, so a
  vacuously-passing oracle would fail.

Past 128 bits the identities carry the weight: `q * b + r == a` with `|r| < |b|`
and `r` signed like the dividend; the arithmetic right shift equalling
`div_euclid` by a power of two (the identity a logical-shift bug breaks
immediately); `!x == -x - 1`; `mod_exp` against a naive repeated-multiply loop
and against Fermat's little theorem on a Mersenne prime; `inv_mod` verified by
multiplying back, with refusals checked against a separate Euclid.

Both no-feature (`no_std`) and rv32 configurations are compiled, and both
256-bit types are driven end to end through a contract entrypoint and
`storage::Value` — in the native harness (`crates/sdk/tests/u256.rs`) and
through a real lowered artifact (`example-contracts/u256_roundtrip`). The
signed cases use a negative value throughout, since sign extension is the bug
they exist to catch.

## Status

Implemented. 166 tests in `crates/u256`, up from 63: 49 differential
(unchanged), 50 signed, 35 math, 15 OpenZeppelin-parity, 6 spec-ABI (was 3),
6 ergonomics, 5 unit. The bench gains four ops -- `mul_div`,
`mul_div_and_div`, `signed_mul`, `signed_div` -- and its backend differential
grows from 16 agreement tests to 22.
Plus four SDK end-to-end cases (was two) and a second lowered-artifact case in
`example-contracts/u256_roundtrip`. `cargo clippy --all-targets`, `cargo fmt
--check` and `cargo doc` are clean, and `groundhog-u256` compiles for
`riscv32im-unknown-none-elf` with no features and with `nightly`.

No new dependencies, so no lockfile changes: the 512-bit test oracle is another
`uint::construct_uint!` on the dev-dependency that was already there.

**Measured.** `scripts/bigint-bench.sh -b u256 -o "div div_small
mul_div_u128"` was run on the pinned CLI (0.9.0) against both this branch and
the base commit, in the same environment; the base run reproduced the recorded
2,640 exactly, which is what makes the comparison sound. Numbers and the
decision they forced are in "One Knuth D body, two widths" above. Net effect on
the pre-existing operations: `div` 68 bytes smaller at +0.9% gas, `div_small`
unchanged in both, `mul_div_u128` 24 bytes smaller and 1.3% faster.

**The new 256-bit `mul_div` has its own bench row.** `uint` ships no
full-precision `mulDiv`, so the row needed a baseline: a
`construct_uint!(PU512(8))` in the parity backend with limb-for-limb widening,
which is what a contract author on that backend would have to write themselves
(`full_mul` exists only for the pairings inside `uint` itself). That makes it a
fair comparison rather than a strawman. `tests/differential.rs` asserts the two
agree before either number is believed.

| operation | `u256` .text | `u256` gas | `uint` .text | `uint` gas |
|---|---|---|---|---|
| `div` | 2,572 | 972.05 | 11,624 | 5002.05 |
| `mul_div_u128` | 3,012 | 1712.05 | 11,280 | 10280.04 |
| `mul_div` | 3,992 | 4712.05 | 21,568 | 22625.04 |

`uint`'s full-precision `mulDiv` costs **21,568 bytes — a third of the 64 KiB
artifact code cap for one operation**, which is the original design's
11,300-byte `Div`/`Rem` problem repeated at double width.

The `mul_div` row sits 1,420 bytes above the `div` row. Read that carefully: it
is *not* "a Knuth D plus `widening_mul_u256`", because `div`'s 2,572 already
contains a Knuth D. By the symbol tables of the two artifacts
(`llvm-readelf -sW target/bigint-bench/u256.{div,mul_div}.elf`), which is what
the unstripped ELFs the bench keeps aside are for:

| | `none` | `div` | `mul_div` |
|---|---|---|---|
| `drive` (the kernel, with Knuth D inlined into it) | 288 | 2,756 | 3,584 |
| `widening_mul_u256` (outlined, per its rv32 `inline(never)`) | — | — | 592 |

`(3,584 - 2,756) + 592 = 1,420`: the 512-bit multiply, plus the wide kernel
being *larger than* the narrow one, not additional to it.

**What these rows do not measure is the cost of the second Knuth D copy.** Each
artifact carries exactly one kernel — `OPS_SELECTED == 1` in `kernel.rs`
enforces it — so neither of these builds links both instantiations. Only a
contract calling both `/` and the full-precision family pays for two, and the
one-kernel-per-artifact design is precisely what makes that unmeasurable here.
Its order of magnitude is the 1,892 bytes one Knuth D costs over
`div_small` (2,572 - 680), but that is an inference from the single-kernel rows,
not a measurement. Measuring it would need a two-kernel probe outside this
harness.

The `div` and `mul_div_u128` rows are byte-identical to the runs before the op
was added, which confirms the new trait method costs nothing to a build that
does not select it — the property the whole one-kernel-per-artifact design
rests on.

### `I256` has bench rows too

`uint` has no signed type at all, so the baseline is the sign-magnitude wrapper
a contract author on that backend would have to write — the same reasoning as
the 512-bit type behind `mul_div`, and checked the same way, by
`tests/differential.rs` asserting the two agree before either number is
believed. The multiply row saturates rather than wraps on purpose: a *wrapping*
signed multiply is bit-identical to the unsigned one, so it would measure `mul`
again, and all of the signed work is in the overflow flag.

| operation | `u256` .text | `u256` gas | `uint` .text | `uint` gas |
|---|---|---|---|---|
| `mul` (unsigned, wrapping) | 564 | 1019.02 | 1,140 | 387.02 |
| `signed_mul` (saturating) | 1,184 | 1095.02 | 3,616 | 793.02 |
| `div` (unsigned) | 2,572 | 972.05 | 11,624 | 5002.05 |
| `signed_div` | 2,848 | 1053.05 | 12,512 | 4800.05 |

Signed arithmetic costs **+620 bytes and +76 gas** over unsigned multiply, and
**+276 bytes and +81 gas** over unsigned division. That is the whole price of
the sign, and it confirms the review's `overflowing_mul` fix: two full
schoolbook passes would have put `signed_mul` near 2,000 gas rather than 1,095.

One row deserves reading honestly rather than selectively. On plain `mul`,
Parity's `uint` is **faster** — 387 gas against 1,019 — while being twice the
size. Sixteen `__multi3` libcalls beat sixty-four inline `mul`/`mulhu` pairs on
these operands. That is a pre-existing row, not something this change moved,
but it is worth stating: the crate's advantage is in *division* and everything
built on it (4–5x on `div`, `signed_div`, `mul_div`), not uniformly everywhere.

## Success criteria

- `I256` agrees with `i128` everywhere both are defined, and its bits agree
  with the unsigned oracle everywhere two's complement says they must. ✅
- `mul_div`/`mul_shr`/`mul_mod` agree with an exact 512-bit oracle across both
  divisor paths, and report overflow rather than truncating. ✅
- A contract that only divides by constants still does not link Knuth D
  (`div_rem_small` unchanged, and `log10` deliberately built on it). ✅
  measured: `div_small` is 680 bytes and 756.04 gas, identical to the
  pre-change baseline.
- `U256` and `I256` differ on the wire in exactly one byte, and that byte is
  the type name. ✅ pinned.
- The library functions answer as OpenZeppelin's do on degenerate inputs. ✅
  pinned case by case in `tests/oz_parity.rs`, each expectation traced from the
  Solidity source; all fifteen passed unchanged.
- Nothing measurable is left inferred. ✅ the second Knuth D copy and the
  signed operations both have bench rows now; the only figures still marked as
  reasoning are the two `math` refactors whose entry points (`mul_shr_rounding`,
  `log10_rounding`) have no row, and that is stated where they are claimed.
