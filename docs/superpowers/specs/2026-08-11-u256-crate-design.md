# `groundhog-u256`: a 32-bit-native 256-bit integer for the SDK

Date: 2026-08-11
Status: design approved, ready for planning

## Problem

Contract authors reach for 256-bit integers for two reasons: parity with
Ethereum, where `uint256` is the default word, and as an exact intermediate
when multiplying two 128-bit values and dividing the product back down.

The current answer is Parity's `uint` crate (`uint::construct_uint!`), used by
the swap suite in `groundhog-examples`. It is correct. It is also badly matched
to our guest targets.

`construct_uint!` stores four `u64` limbs and reaches for `u128` intermediates
in every widening multiply. On riscv32 a `u64 x u64 -> u128` is not two
instructions; it is a `compiler_builtins` call with the `u128` passed
indirectly through memory. Every multiply on the hot path pays call overhead
and spills rather than arithmetic. The same applies on wasm32, which has native
`i64` but no native `i128`.

The size cost is already measured. From `swap-u256`'s own crate docs:

- `uint`'s Knuth-D `Div`/`Rem` over four 64-bit limbs lowers to **11,300 bytes**
  on rv32 — 17% of the artifact format's 64 KiB code cap, for a contract that
  divides three times.
- `uint`'s Newton-iteration `integer_sqrt` lowers to **12,732 bytes**, because
  each iteration step is a full 256-bit division that LLVM then inlines into
  the sqrt loop.

The SDK designer already worked around both, by hand-rolling `div_rem` (Knuth
algorithm D, base 2^32) and `integer_sqrt` (binary restoring) over `[u32; 8]`
inside `contracts/swap-u256`, converting to and from `uint`'s `[u64; 4]` at
each call boundary. That work is proven and well tested. It is in the wrong
place — an example contract — and it still pays a limb-conversion round trip on
every call because the public type's representation is still `[u64; 4]`.

## Goals

- A 256-bit unsigned integer whose internal representation is native to a
  32-bit register file.
- Idiomatic operators, so ordinary Rust code (`a + b`, `a * b`, `a / b`,
  comparisons) simply works. This is a usability requirement, not a nicety:
  authors reaching for U256 are porting Ethereum mental models.
- No `compiler_builtins` 128-bit libcalls on any hot path.
- Code size discipline strong enough that a contract which only needs cheap
  operations does not link the expensive ones.

## Non-goals

- `I256`. Signed 256-bit arithmetic is wanted eventually, but nothing in the
  swap suite uses it today, so there is no call site to shape the API against —
  in particular floor-vs-truncate division semantics, and which of
  `abs`/`signum`/`checked_neg` matter. Deferred until a consumer exists.
  (**Since shipped**: a consumer appeared. See
  [`2026-08-31-i256-and-math-utilities-design.md`](2026-08-31-i256-and-math-utilities-design.md).)
- A general `mul_div(U256, U256, U256)`. Exactness requires a 512-bit
  intermediate. `mul_div_u128` is exact with a U256 intermediate and covers the
  motivating case; the general form is deferred rather than shipped lossy.
  (**Since shipped**, alongside `I256` and the rest of OpenZeppelin's
  `Math.sol`; same design note.)
- `pow`, modular arithmetic, Montgomery form, constant-time guarantees. We are
  not building a cryptographic bignum.
- Const-generic width (`Uint<const LIMBS>`). Const generics invite accidental
  monomorphization, which is precisely the wrong failure mode under a 64 KiB
  code cap.

## Alternatives considered

**Fork `uint` and retarget its limbs to `u32`.** `construct_uint!` assumes
`u64`/`u128` in substantially every arm; retargeting it is a rewrite wearing a
macro's clothes. It also inherits the macro's habit of emitting the full
numeric tower whether or not the contract calls it, which is the code-size
problem we are trying to solve. Rejected.

**Adopt `crypto-bigint`.** Genuinely tempting: its `Limb` is pointer-width, so
rv32 gets `u32` limbs for free. Three problems. Its operations are
constant-time by construction, which we do not need and would pay for in a gas
model. Its limb width keys off `target_pointer_width`, which happens to suit us
but is not a choice made for our reasons. Decisively: `Uint<LIMBS>` is a
foreign type, so contract-type integration requires a newtype wrapper
regardless — at which point the wrapper is written and the only remaining
question is what sits inside it. Rejected.

**Roll our own, seeded by `swap-u256`.** Chosen. We are not starting from zero:
the two functions that account for ~24 KB of the problem already exist as
tested `[u32; 8]` implementations. Making `[u32; 8]` the native representation
additionally deletes the `to_limbs`/`from_limbs` round trip that every current
call pays.

Other crates surveyed and rejected: `ruint` (hardcoded `u64` limbs, same
disease as `uint` with a larger surface), `ethnum` (built on `u128`, worst case
for rv32), `bnum` (`u64` digits), `fixed-bigint` (right shape, thin operation
coverage, not tuned).

## Design

### Crate placement

New workspace member `crates/u256`, package name `groundhog-u256`,
re-exported as `sdk::U256`.

A separate crate rather than a module inside `crates/sdk`, for the reason
`swap-u256`'s crate docs give: 256-bit division is the largest single item in a
contract's rv32 binary, and only that crate needs a size-tuned profile. Keeping
it a distinct package preserves per-package `[profile.*.package]` overrides.

Must be added to the root `Cargo.toml` `default-members` list, per the comment
there — it is not a target-only runtime crate.

**Constraint discovered during design, since resolved.** `crates/macros`
hard-coded `::sdk::` paths with no crate-path override, so a `crates/u256`
using `#[sdk::contract_type]` would have been a dependency cycle
(`u256 -> sdk -> u256`). The first cut of this crate therefore hand-wrote the
spec impls.

PR #337 added `#[contract_type_spec(crate = "...")]` to
`#[derive(ContractTypeSpec)]`. Nothing that derive emits is owned by `sdk` —
it reaches only for `__contract_spec` and `__xdrs`, both `sdk` re-exporting a
crate below it — so a path shim is enough. `crates/u256` now provides
`__sdk_paths` re-exporting those two names and derives its spec impls, keeping
only `ContractTypeName` by hand (the derive references it but never emits it;
it is a name string with no encoded bytes).

`tests/spec_abi.rs` pins the result byte-for-byte against the literals the
hand-written version produced, so the swap is verified rather than assumed.

The remaining trait set still splits by ownership:

| Trait | Defined in | Implemented in |
|---|---|---|
| `ReadXDR`, `WriteXDR` | `xdrs` | `crates/u256` (hand-written: MS-first limb order) |
| `ContractTypeSpec`, `ContractTypeRefSpec` | `groundhog-contract-spec` | `crates/u256`, via the derive |
| `ContractTypeName` | `groundhog-contract-spec` | `crates/u256`, by hand |
| `CollectAuthHandles`, `ReceiveAuthHandles`, `MintableType` | `crates/sdk` | `crates/sdk` (local trait, foreign type) |

The full `#[contract_type]` attribute macro is still unavailable below `sdk`,
because it also emits impls of the sdk-owned traits in the last row — a
dependency problem rather than a path one. Not needed here: those three impls
already live in `sdk`, where `U256` joins the existing `impl_collect_noop!` /
`impl_receive_noop!` / `impl_mintable_leaf!` lists.

Both directions are orphan-safe: the `sdk` half is a local trait on a foreign
type, the `u256` half a foreign trait on a local type.

### Representation

```rust
pub struct U256 { limbs: [u32; 8] }   // limbs[0] is least significant
```

A named field rather than a tuple: `#[derive(ContractTypeSpec)]` names an
unnamed field `_0`, and the spec field name is wire-visible, so the Rust-side
field carries `limbs`. `from_limbs`/`to_limbs` are the accessors either way.

`Copy + Clone + PartialEq + Eq + PartialOrd + Ord + Hash`. `Copy` is required:
`swap-pair`'s `Reserves` derives `Copy` and holds `[U256; 2]`.

`[u32; 8]` unconditionally — no cfg on limb width. On wasm32, `u32 x u32 -> u64`
lowers to `i64.extend_u` plus `i64.mul`, both native and cheap, so `u32` limbs
tie or beat `u64` limbs there, and both beat `uint`'s `__multi3` path. The
native host build is used only for tests and simulation, where the constant
factor does not matter. One implementation, one test surface.

Limb order is least-significant-first because the `swap-u256` code being ported
is written that way. Knuth D is fiddly enough that reversing every index during
a port is a real correctness risk for no benefit.

### Wire format

Encoded as eight `u32` limbs, **most significant first**, matching the
convention xdrs already uses for `u128` — `xdrs-0.8.2/src/int128.rs` writes
`(high u64, low u64)`. Since XDR integers are themselves big-endian, the
resulting 32 bytes are the value in big-endian byte order, which is also
Ethereum's word order and what any non-Rust SDK would guess.

Spec type is a named struct with a single `Array(U32, 8)` field. No change to
`ContractTypeView`, no hogd change, no CLI change.

`ReadXDR`/`WriteXDR` (including `write_xdr_unchecked`) are written by hand, but
only to walk the limb array in reverse — the internal order stays LS-first.
Roughly eight lines per direction.

Pre-launch, so no migration concern: this differs from `uint`'s current
`Array(U64, 4)` encoding and that is fine.

### Operation surface

Two tiers, split deliberately.

**Idiomatic tier** — present so that ordinary code works without the author
learning a vocabulary:

- `Add`, `Sub`, `Mul`, `Div`, `Rem` and their `*Assign` forms
- `Shl<u32>`, `Shr<u32>`, `BitAnd`, `BitOr`, `BitXor`, `Not`
- `Ord`, `Eq`, `Hash`
- `From<u8>`, `From<u16>`, `From<u32>`, `From<u64>`, `From<u128>`;
  `TryFrom<U256> for u128` and `for u64`
- `checked_`, `wrapping_`, `overflowing_`, `saturating_` variants of
  add/sub/mul
- Associated consts `ZERO`, `ONE`, `MAX`, `BITS`; methods `is_zero`,
  `bit_len`, `leading_zeros`

**Fast tier** — the reason the crate exists:

- `U256::widening_mul(a: u128, b: u128) -> U256` — sixteen unrolled
  `mul`/`mulhu` pairs, no calls, no libcalls. This is the dominant shape at
  real call sites.
- `U256::mul_u32(self, rhs: u32) -> U256`
- `U256::div_rem(self, rhs: U256) -> (U256, U256)` — ported Knuth D
- `U256::div_rem_small(self, rhs: u32) -> (U256, u32)`
- `U256::integer_sqrt(self) -> U256` — ported binary restoring
- `mul_div_u128(a: u128, b: u128, d: u128) -> u128` — exact; the
  motivating i128-times-i128 case end to end, with the U256 never escaping
  into user code. Plus a `checked_mul_div_u128` returning `Option`.

The operator tier is a thin routing layer over the fast tier, not a second
implementation.

### The code-size rule

`div_rem_small(U256, u32)` is a **separate function** from
`div_rem(U256, U256)`, not a fast path inside it. Knuth D is the 11 KB item; a
contract that only divides by constants must be able to link the small path
without dragging in the general one. `Display` in particular divides by 10^9
repeatedly and must not pull Knuth D into a build that formats.

The same split applies to `Mul<u32>` versus `Mul<U256>`, and `widening_mul`
versus the general schoolbook 256x256.

Verification of this rule is part of the test plan, not a hope: see below.

### Overflow semantics

The operators behave exactly as `u128` does: they wrap when `overflow-checks`
is off and panic when it is on. `Div` and `Rem` panic on a zero divisor
regardless, again matching the primitives.

An earlier draft had the operators panic on overflow unconditionally, on the
theory that this was the safe default. That was wrong for this codebase. Every
contract profile sets `overflow-checks = false` (`.cargo/contract.toml:19`,
`example-contracts/.cargo/*.toml`, `integration_test/.cargo/*.toml`), so
`u128` arithmetic in a release contract build wraps silently. A `U256` that
panicked would be *stricter than the primitive sitting next to it in the same
expression* — a surprise in the direction that matters, since authors reach for
`U256` precisely to hold intermediates of `u128` math.

Detection uses `cfg(overflow_checks)`, the flag rustc itself keys the
primitives off, written as `cfg!(overflow_checks) && overflowed` so the branch
and its message const-fold away in a release build. This needs
`#![feature(cfg_overflow_checks)]` — the first nightly language feature in a
published crate here, though the toolchain is already pinned to nightly for
`-Z build-std`. `cfg(debug_assertions)` is a faithful stable substitute under
every profile in this workspace (all of them move the two flags together) and
the swap is one line per site; the code says so at the `#![feature]`.

Authors who want to handle overflow explicitly use the
`checked_`/`overflowing_`/`saturating_` variants.

No new `LibraryExn` variant is involved. Adding one would have meant an exn
version bump and a change to the hand-rolled `ContractExn<LibraryExn>` encoder
landed in PR #279. Where a panic does fire, rv32 lowers it to a single `ebreak`
(`crates/rv32-runtime/src/panic.rs`), which hogd traps as `Trap::kBreakpoint`;
wasm builds with `-Cpanic=immediate_abort`.

### Formatting

`Debug` is implemented unconditionally, rendering hexadecimal via its own
inline nibble extraction — it does not route through the `LowerHex` impl, so it
stays available when that impl is compiled out. It must be unconditional
because `swap-pair`'s `Reserves` does `#[derive(Debug)]` over `[U256; 2]`
(`contracts/swap-pair/src/reserves.rs:13`), and gating it would break that
derive. Hex costs nibble extraction and no division.

`Display`, `FromStr`, `LowerHex` and `UpperHex` are gated behind a `fmt`
feature on `groundhog-u256`. `Display` and `FromStr` are the expensive pair —
they need the repeated decimal division loop; `LowerHex`/`UpperHex` ride along
because nothing in a release guest build should be formatting anyway.

`sdk`'s `std`, `dev`, `mock` and `debugging` features each enable
`groundhog-u256/fmt`; the crate also enables it under `cfg(test)`. A single
`fmt` feature rather than mirroring four SDK features keeps the wiring legible.

Release guest builds therefore carry neither the decimal loop nor its rodata.

## Testing

**Differential against `uint`.** Keep Parity's `uint` as a **dev-dependency
only**, as an oracle. This is what `swap-u256`'s existing tests already do —
they assert `integer_sqrt` and `div_rem` match `uint`'s implementations across
boundaries, powers of two, and deterministic xorshift sweeps sized to actually
exercise Knuth D's add-back correction (which fires with probability ~2/2^32
and needs volume to be hit at all). Port those tests as-is and extend the same
treatment to add, sub, mul, and the shifts.

**Against `u128`.** Every operation whose operands and result fit in 128 bits
must agree with the corresponding `u128` operation.

**Round-trip.** XDR encode/decode round-trips, plus a fixed vector asserting
that a known value encodes to the expected 32 big-endian bytes, so the
MS-first limb order is locked against accidental reversal.

**Code-size rule.** A test contract that uses only `div_rem_small` must not
link Knuth D. Assert by symbol inspection of the built rv32 artifact, alongside
the existing rv32 metrics CI (PRs #275, #329).

**No libcalls.** Assert the swap fleet's rv32 disassembly contains no
`__multi3` or `__muldi3`.

## Migration

1. Land `crates/u256` with tests green, and add it to the rv32 metrics fleet.
2. Repoint `groundhog-examples`' `swap-u256` at `sdk::U256`; delete its local
   Knuth D, `integer_sqrt`, limb-conversion helpers, and its `uint` dependency.
3. Record swap-pair rv32 `.text` before and after.

Note two housekeeping items. First, `groundhog-examples` is a separate
workspace, so this is a cross-repo change sequenced after the SDK release that
carries `sdk::U256`. Second, adding a crate and its dependency edges requires
syncing all six `Cargo.lock` files in this repo — CI's `--locked` builds fail
per-lockfile otherwise, and `integration_test` carries its own hard pins.

While in `swap-u256`, fix its stale doc comment: `div_rem`'s comment describes
binary restoring division ("up to 256 iterations rather than a limb-at-a-time
quotient estimate") but the body is Knuth algorithm D with a single-limb `DIVU`
fast path — the opposite algorithm with the opposite size/speed tradeoff.
`integer_sqrt`'s comment is accurate.

## Status

Implemented on `geoff/u256-crate`. Measured as built, rv32,
`opt-level = "z"`, LTO, `codegen-units = 1`:

- Reaching for the general `U256 / U256` adds **2,044 bytes** of `.text` over a
  build that only calls `div_rem_small`. Two things follow: the split works
  (the small path does not drag Knuth D in), and Knuth D over `[u32; 8]` costs
  roughly a sixth of `uint`'s 11,300-byte equivalent.
- 54 tests pass (differential against `uint`, direct coverage of the 64-by-32
  divide, and the spec-ABI pins); `sdk` is unchanged at 33 passing.
- Both guest targets compile `no_std`; `UpperHex`'s digit table and the decimal
  buffer length both had to be feature-gated, which is direct evidence the
  `fmt` gating removes what it claims to.

### `DIVU` is XLEN-wide (found in review)

The first draft of `div_rem_small` wrote its per-limb step as
`((rem as u64) << 32 | limb as u64) / d as u64`, with a comment claiming this
lowered "straight onto rv32's `DIVU`". That was wrong, and the claim was
inherited from the `swap-u256` code this crate ports.

`DIVU` is XLEN-wide, so on a 32-bit target it divides 32-bit operands only —
there is no 64-by-32 divide to lower onto. Disassembly confirmed it: the
function contained **zero** `divu` instructions and the build pulled in
`__udivdi3`/`__umoddi3`/`__udivmoddi4`. Eight limb iterations meant eight
compiler-builtins calls, in the path this design calls cheap.

Replaced with `div_rem_2by1`, Hacker's Delight's `divlu` — schoolbook division
in base 2^16, so every divide is a native 32-bit `DIVU`. After: **2 `divu`
instructions, zero libcalls.** The routine has a precondition (`hi < d`) that
holds at the call site because `rem` is a remainder, and it is covered
directly by 250,000 randomized cases against the `u64` spelling it replaces,
plus the normalised/unnormalised divisor edges.

**Known follow-up.** Knuth D's quotient estimate has the same shape
(`num / vn[dn-1]` over a `u64`) and still emits one libcall per quotient limb —
confirmed by disassembly. It was left alone deliberately: Knuth D permits
`un[j+dn] == vn[dn-1]`, so `qhat` can reach 2^32 and `div_rem_2by1`'s
precondition does not hold; handling that requires reworking the correction
loop into 32-bit arithmetic. That is a delicate change to the least-used path,
and it did not belong in the same commit as the crate's introduction.

## Success criteria

Met here:

- Differential tests against `uint` pass across the full ported sweep, plus new
  sweeps for add/sub/mul/shift/bitwise/format.
- A contract using only `div_rem_small` does not link Knuth D (measured above).

Deferred to the `groundhog-examples` migration, which sequences after an SDK
release carrying `sdk::U256`:

- swap-pair rv32 `.text` shrinks against the recorded baseline.
- No `__multi3`/`__muldi3` in the swap fleet's rv32 disassembly.
- `swap-u256` contains no arithmetic of its own.

Follow-up: the code-size rule is currently verified by hand. Turning it into a
standing assertion in the rv32 metrics CI (PRs #275, #329) is worth doing, but
it needs a cross-compiled fixture and did not belong in this change.
