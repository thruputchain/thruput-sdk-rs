# `LibraryExn::UnknownMethod`: a typed exception for undefined methods

Status: implemented, unconditional.

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

It is not behind a feature. It was drafted as one (`unknown_method_exn`),
but the measurement below came in at 20 bytes of `.text` and nothing in the
stored data image, which is not worth a cfg axis: two dispatch behaviors to
document, test, and reason about across every contract build.

### Layering

`groundhog-rv32-runtime` is dependency-free by design and `sdk` depends on
*it*, so the runtime cannot build a `LibraryExn`. They meet at one C symbol:

- the runtime declares `__groundhog_unknown_method` and tail-calls it in
  place of the old `ebreak` (`crates/rv32-runtime/src/start.rs`);
- `sdk::__rv32` defines it, as `library_throw(LibraryExn::UnknownMethod)`.

Nothing links the runtime without `sdk` — `sdk` force-links it on rv32 — so
the definition is always present.

The native mock executor mirrors this so a contract's `cargo test` agrees
with its deployed image: `MockContract::register_unknown_method` installs a
fallback that `trampoline` runs when no method matches, and
`register_mock_package` installs the throwing one. Hand-built mocks
(`env.register_mock`) are unaffected — they define exactly the methods they
register, and are not images of a contract.

### No `method_id` payload

The variant is a unit. The caller chose the id it invoked, so echoing it
back buys nothing at the only place that can read it, and a unit variant
keeps both the encoded payload and the emitted code minimal. Adding a
payload later is a wire break; adding a *new* variant is not.

### Wire compatibility

Discriminant 13 is additive, and `LibraryExn`'s wire layout stays fixed
regardless of crate features (see the type's doc comment). An older client
decoding a `LibraryExn` it has never seen fails to decode that payload,
exactly as it would for any newer variant — the visible change is that a
contract which used to answer an undefined method with a bare trap now
answers with a decodable exception.

## Codesize

Measured on the `example-contracts` rv32 suite, built the way CI builds it
— one `cargo build -p <package>` per contract, release, `--icf=all`,
`panic=immediate-abort`. (Building the whole workspace in one invocation
unifies `sdk` features across every contract — `exceptions` alone drags
`debugging` + `alloc` into all of them — and inflates every number by 2-4x.
Measure per package.)

| | `.text` | `data_image` |
|---|---|---|
| every contract | **+20 B** | +0 B |

Plus, for declaring the variant at all, +0..+8 B of `data_image` (mean
+1.6 B) in the contracts whose published spec names `LibraryExn` — the spec
fragment for the new variant. Declaring it costs no code: no release build
formats or matches on it.

The 20 bytes are `__groundhog_unknown_method` itself — `li a0, 13` plus a
call to the already-linked `library_abort`, which every contract carries
anyway for storage getters and syscall wrappers. Nothing else moves: the
dispatcher's `ebreak` becomes a tail call, and the exception encoder gains
no arm (a unit variant is a bare discriminant). Range over the suite: 0.1%
of `.text` on `erc20_raw_storage` (18 KB) to 1.3% on the smallest fixtures
(~1.6 KB). One contract, `raw_iface_dep`, comes in at +16 B; it is the
hand-written raw contract with no `#[contract]` macro.

## Fixture

`example-contracts/unknown_method` carries the native test that a call to
an undefined entrypoint id comes back as `LibraryExn::UnknownMethod`.
