# `unknown_method_exn`: a typed exception for undefined methods

Status: implemented, off by default.

## The gap

The hogd VM enters every call at `_start` with the invoked `method_id` in
`a0`/`a1`. `rv32_dispatch` scans the `groundhog_rv32_entrypoints` table and,
finding no match, executed `ebreak` — trapped as `Trap::kBreakpoint`, a
deterministic failure with an empty return buffer.

That is correct but mute. "This contract has no such method" is
indistinguishable, at the caller, from an out-of-bounds access, a bad
instruction, or any other trap. A client that calls a contract it does not
control — a proxy, a router, a version probe — cannot tell a missing
entrypoint from a broken one.

## What was added

`LibraryExn::UnknownMethod = 13`, thrown by the dispatcher instead of
trapping. Being a `LibraryExn`, it reaches the caller with the library
selector and decodes through the existing path (`ClientError::Library`,
`RawExceptionRef::try_as::<LibraryExn>`) — no new client machinery.

It is gated on the SDK's `unknown_method_exn` feature, **off by default**:
the old behavior is what deployed contracts have, and switching is a
wire-visible choice about what a contract's callers observe, not a
free upgrade.

### Layering

`groundhog-rv32-runtime` is dependency-free by design and `sdk` depends on
*it*, so the runtime cannot build a `LibraryExn`. They meet at one C symbol:

- the runtime declares `__groundhog_unknown_method` and tail-calls it in
  place of `ebreak` (`crates/rv32-runtime/src/start.rs`);
- `sdk::__rv32` defines it, as `library_throw(LibraryExn::UnknownMethod)`.

The feature therefore has to be enabled on `sdk` (which forwards it to the
runtime), never on the runtime alone — the runtime alone links to an
undefined symbol, which is the intended failure.

The native mock executor mirrors this so a contract's `cargo test` agrees
with its deployed image: `MockContract::register_unknown_method` installs a
fallback that `trampoline` runs when no method matches, and
`register_mock_package` installs the throwing one under the same feature.
Hand-built mocks (`env.register_mock`) are unaffected — they define exactly
the methods they register, and are not images of a contract.

### No `method_id` payload

The variant is a unit. The caller chose the id it invoked, so echoing it
back buys nothing at the only place that can read it, and a unit variant
keeps both the encoded payload and the emitted code minimal. Adding a
payload later is a wire break; adding a *new* variant is not.

### Wire compatibility

`LibraryExn`'s wire layout is fixed regardless of crate features (see the
type's doc comment), so the variant is declared unconditionally — only the
*throwing* is gated. Discriminant 13 is additive: an older client decoding
a `LibraryExn` it has never seen fails to decode that payload, exactly as it
would for any newer variant.

## Codesize

Measured on the `example-contracts` rv32 suite, built the way CI builds it
— one `cargo build -p <package>` per contract, release, `--icf=all`,
`panic=immediate-abort`. (Building the whole workspace in one invocation
unifies `sdk` features across every contract — `exceptions` alone drags
`debugging` + `alloc` into all of them — and inflates every number by 2-4x.
Measure per package.)

Three configurations: `main`; the variant added with the feature off; the
feature on.

| | `.text` | `data_image` |
|---|---|---|
| declaring the variant (feature off) | **+0 B**, every contract | +0..+8 B (mean +1.6 B) |
| enabling `unknown_method_exn` | **+20 B**, every contract | +0 B |

`+20 B` is `__groundhog_unknown_method` itself — `li a0, 13` plus a call to
the already-linked `library_abort`, which every contract carries anyway for
storage getters and syscall wrappers. Nothing else moves: the dispatcher's
`ebreak` is replaced by a tail call, and the exception encoder gains no
arm (a unit variant is a bare discriminant). Range over the suite: 0.1% of
`.text` on `erc20_raw_storage` (18 KB) to 1.3% on the smallest fixtures
(~1.6 KB). One contract, `raw_iface_dep`, comes in at +16 B; it is the
hand-written raw contract with no `#[contract]` macro.

Declaring the variant costs nothing in code because no contract's release
build formats or matches on it; the few `data_image` bytes are the spec
fragment, in contracts that publish a spec naming `LibraryExn`.

## Fixture

`example-contracts/unknown_method` is the one contract in the tree built
with the feature on. It carries the native test that a call to an undefined
entrypoint id comes back as `LibraryExn::UnknownMethod`, and its codesize
row is the standing measurement.
