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
- `Math.modExp`'s arbitrary-length `bytes` variant. It exists in Solidity
  because the EIP-198 precompile takes arbitrary-length inputs; there is no
  precompile here, and no consumer.
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
- `MIN / -1`, `MIN.abs()` and `-MIN` overflow; they panic under
  `overflow-checks` and wrap without it, exactly as the `U256` operators do.
  `unsigned_abs` is the total version, and returns a `U256` — the same shape
  `SignedMath.abs` has.

**Wire format.** 32 big-endian two's complement bytes: byte for byte what a
`U256` with those bits encodes, and what Ethereum's `int256` uses. The spec
type is a named struct `I256` with one `Array(U32, 8)` field, so `U256` and
`I256` differ on the wire in exactly one byte — the first character of the type
name. `tests/spec_abi.rs` pins that, including the one-byte claim.

A distinct spec name rather than a shared one is the whole safety property
here: a decoder that confused them would read the same 32 bytes with the wrong
sign.

### One Knuth D, two widths

`Math.mulDiv` needs a 512-bit dividend. The choice was between a second copy of
Knuth algorithm D specialised to sixteen limbs, and generalising the existing
one.

Generalising won. `div_rem_knuth` now takes the dividend as a limb *slice* with
an explicit length, and is called by both the `U256 / U256` operators (eight
limbs) and `div_rem_512` (sixteen). Every loop in it is bounded by the runtime
`nn`/`dn`, so the wider buffers cost the narrow caller ~64 bytes of stack and
no instructions — where a second copy would have cost both, on the crate's most
delicate routine.

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

Implemented. 144 tests in `crates/u256`, up from 63: 49 differential
(unchanged), 32 math, 46 signed, 6 spec-ABI (was 3), 6 ergonomics, 5 unit.
Plus four SDK end-to-end cases (was two) and a second lowered-artifact case in
`example-contracts/u256_roundtrip`. `cargo clippy --all-targets`, `cargo fmt
--check` and `cargo doc` are clean, and `groundhog-u256` compiles for
`riscv32im-unknown-none-elf` with no features and with `nightly`.

No new dependencies, so no lockfile changes: the 512-bit test oracle is another
`uint::construct_uint!` on the dev-dependency that was already there.

**Not measured here.** The original design's rv32 codesize numbers came from
`dev bigint-bench`, which needs the `groundhog` CLI. The generalisation of
Knuth D is expected to be codesize-neutral (identical instructions, wider stack
frames), but that is reasoning, not a measurement, and it should be confirmed
with `dev bigint-bench -o 'div div_small'` before release. The benchmark
harness itself is unchanged — it measures one kernel per artifact against
Parity's `uint`, which has no signed counterpart to compare against.

## Success criteria

- `I256` agrees with `i128` everywhere both are defined, and its bits agree
  with the unsigned oracle everywhere two's complement says they must. ✅
- `mul_div`/`mul_shr`/`mul_mod` agree with an exact 512-bit oracle across both
  divisor paths, and report overflow rather than truncating. ✅
- A contract that only divides by constants still does not link Knuth D
  (`div_rem_small` unchanged, and `log10` deliberately built on it). ✅ by
  construction; ⏳ pending a `bigint-bench` measurement.
- `U256` and `I256` differ on the wire in exactly one byte, and that byte is
  the type name. ✅ pinned.
