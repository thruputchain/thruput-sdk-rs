# Hand-rolled `LibraryExn` abort encoder

**Date:** 2026-07-09
**Status:** Approved design, pre-implementation

## Problem

`encode_scratch::<ContractExn<LibraryExn>>` (reached via
`_contract_abort_encoded`) is the single largest function in every
contract: ~2.2 KB in the counter rv32 ELF
(`_contract_abort_encoded::<ContractExn<LibraryExn>>` = 1228 B,
`SyscallExn::write_xdr::<CXS, IntoBufWriter>` = 588 B, plus the shared
380 B `IntoBufWriter::write_u64`), roughly 22% of counter's `.text`.
Every contract pays it because every storage getter, syscall wrapper,
`re_throw`, and `none_if_missing` can abort with a `LibraryExn`.

Disassembly shows where the bytes go:

1. The `u64` selector — a compile-time constant — is varint-encoded at
   runtime through the generic writer.
2. Every field write carries generic-writer plumbing: load ptr/cap/len
   from a stack struct, bounds-check with an error branch, store, write
   the length back, check a `WriteResult` sentinel (~10 instructions of
   ceremony per byte, ~30 field writes across the variant arms).
3. The varint width cascade (`write_i32` → `write_i16` → `write_i8`)
   inlines per field; `SyscallExn::write_xdr` is 588 B to encode two
   small integers.
4. The fallible-encode machinery: a 1040 B stack frame, two 504 B
   `memcpy`s for the `with_scratch` dance, and the cold
   `exception_encoding_error` fallback with its fresh 4 KB buffer.

With `FaultString` a ZST in release (no `debugging` feature), every
`LibraryExn` variant encodes to ≤ 28 bytes. Encoding a bounded value
into a bounded buffer does not need any of the above.

## Goals

- Remove the `encode_scratch::<ContractExn<LibraryExn>>` /
  `_contract_abort_encoded::<ContractExn<LibraryExn>>` monomorphization
  from contract builds; target ~1.3–1.6 KB off counter's `.text`,
  measured with `scripts/metrics/sdk_codesizes_rv32.sh`.
- Byte-identical wire output to the derived encoder in both cfg modes
  (`debugging` on and off), enforced by tests.
- No new CXS varint implementation in the SDK: all variable-width
  integer encoding goes through the existing public
  `xdrs::cxs::encode_into`.

## Measured outcome (2026-07-09, post-implementation)

The size target above was estimated against a pre-#275 baseline; #275's
`--icf=all` had already deduplicated the derived encoder's worst
redundancy (the 588 B `SyscallExn::write_xdr` copy et al.), so the true
available win was ~1.5 KB of derived-encoder cost, not 2.2 KB. Measured
on counter (`text=`, baseline main `5c7134a` = 6008 B):

1. Per-field `encode_into` (the "no new varint code" goal): **5936 B,
   −72 B net.** `library_abort` came out at 1200 B — the per-call
   writer setup, subslice bounds/`WriteResult` plumbing, and inlined
   width cascades reproduce the derived encoder's cost almost exactly.
   The third goal above was abandoned on these numbers (per the Risks
   fallback), with user sign-off.
2. Local hand-written varint puts (`put_u64`/`put_i32`), `FaultString`
   still via `encode_into`: 5788 B, −220 B.
3. Plus `#[inline(never)]` on the two helpers (LLVM was duplicating the
   width cascades into ~5 match arms while also emitting the outlined
   copies): **5612 B, −396 B net (−6.6%).** Final shape:
   `library_abort` 672 B + `put_u64` 276 B + `put_i32` 208 B, vs the
   derived path's ~1.5 KB.

The wire-compat suite (byte-equality vs. the derived oracle across all
variants and integer-width boundaries) pinned every iteration; the
qualitative goals (no per-contract monomorphization, statically
infallible abort path, no 4 KB fallback buffer, `re_throw` 84→4 B)
held throughout.

## Non-goals

- User exceptions (`throw<E>`): `ContractExn<UserExn>` keeps its per-`E`
  monomorphization through the existing generic path.
- Changing the wire format, the derived `WriteXDR`/`ReadXDR` impls on
  `LibraryExn` (clients, native-exec, and spec tooling still use them),
  or anything in xdrs.
- Return/event/storage encoding paths.

## Design

New module `crates/sdk/src/exn/abort_encode.rs`.

### Encoder

```rust
/// CXS-encode `ContractExn<LibraryExn>` — [selector][discriminant][payload]
/// — into a fixed buffer. Byte-compatible with the derived encoder
/// (pinned by tests); exists so contracts don't monomorphize the
/// generic encode machinery for the SDK's own abort path.
fn encode_library_abort(exn: &LibraryExn, buf: &mut MUBuf<ABORT_MAX>) -> usize
```

- Selector: `encode_into(&LibraryExn::SELECTOR, …)` — runtime encode
  through the shared primitive; no const-encoding machinery.
- Outer and nested enum discriminants (`LibraryExn` 0..=11,
  `HandlingError`/`CallingError`/`AuthTokenError` 0..=3): stored as
  single bytes directly. This is the only CXS format knowledge the
  encoder owns — a non-negative XDR int ≤ 30 encodes as itself in the
  header byte — and the wire-compat tests pin it.
- Runtime integer fields (`SyscallNo`, `SysErr`, the `u32`/`u64` size
  pairs): one `encode_into(&field, …)` call each, delegating to the
  field's own `WriteXDR` impl. These land on the shared outlined
  `IntoBufWriter` primitives that no-alloc contracts already link for
  storage/return encoding, so the marginal code is near zero.
- `FaultString` fields: `encode_into(&fs, …)`. In release the impl is
  `#[inline(always)] Ok(())` and compiles to nothing; under `debugging`
  it writes the real `Option<str>`. One encoder, both cfgs.
- Buffer: `MUBuf<ABORT_MAX>` on the stack (`spare_capacity_mut()` feeds
  `encode_into`; direct `MaybeUninit::new` stores for discriminant
  bytes; `set_len` at the end). `MUBuf` is exported unconditionally, so
  this works in both `alloc` and no-alloc builds.
- `ABORT_MAX`: 32 in release (worst case is 28 B:
  9 selector + 1 discriminant + 2×9 for
  `StorageIndexOutOfBounds{u64,u64}`); 544 under `debugging`
  (≤ 515 B with a full 500-char `FaultString`). cfg-selected const.
- Fallibility: `encode_into`'s `WriteResult` is statically
  unsatisfiable given the sizing (both bounds are compile-time
  constants), but the `Result` must be consumed; each call gets
  `.unwrap_or_else(|_| crate::trap())` — a cold branch, same last-resort
  behavior as today's double-failure path. A unit test encodes the
  max-width instance of every variant and asserts the length fits
  `ABORT_MAX` in both cfg modes.
- `SyscallExn`'s `errno` field is private to `library.rs`; add a
  `pub(crate)` accessor (or `pub(crate)` fields) so the encoder can
  reach it.

### Abort entry points

```rust
#[cfg_attr(feature = "debugging", track_caller)]
fn library_abort(exn: &LibraryExn) -> !        // encode + contract_abort
fn client_library_abort(exn: &LibraryExn) -> ! // encode + client abort path
```

The `debugging` + `std` `abort_breadcrumb` call and the `track_caller`
chain move into these, so dev diagnostics are unchanged.

### Rerouting — the complete `ContractExn<LibraryExn>` kill list

Exactly three sites encode `ContractExn<LibraryExn>` today; all reroute
to the new encoder:

1. `library_throw` (exn/library.rs:168) — also covers `re_throw`,
   `none_if_missing`, and every `syscall_throw`/`nnint_*`/`encoding_*`
   helper, which all bottom out here.
2. `client_library_throw` (exn/library.rs:174) — covers generated
   client code (`macros/src/common/client.rs` imports it as `cthrow`).
3. `exception_encoding_error` (contract_op.rs:118) — reroutes its
   `LibraryExn` encode through the new encoder, killing the fresh-4KB
   buffer dance (the new encode never touches the shared scratch and
   cannot fail). The function remains as the fallback for *user* exn
   encode failures in `throw<E>`.

`ContractExn` itself, `_contract_abort_encoded`, and `encode_scratch`
are unchanged — they still serve `throw<E>` for user exception types.

## Testing

- **Wire-compat (primary):** for every `LibraryExn` variant — including
  every nested `HandlingError`/`CallingError`/`AuthTokenError` variant —
  and varint width boundaries on runtime fields (0, the 30/31 header
  edge, i8/i16/u8/u16/u32/u64 edges, `u64::MAX`), assert byte-equality
  of `encode_library_abort` output against
  `encode_xdr(&ContractExn::new(exn))` (the derived encoder as oracle).
  Lives in the sdk's normal test build (`dev` features, `debugging` on,
  so the `FaultString` string arm is exercised; the release arm differs
  only by `FaultString`'s own ZST impl, which has its own roundtrip
  tests in exn/fault.rs).
- **Sizing:** max-width instance of every variant fits `ABORT_MAX`
  (both cfg modes).
- **Growth tripwire:** the encoder's match is exhaustive
  (`#[non_exhaustive]` does not relax same-crate matches), so adding a
  `LibraryExn` variant fails to compile until the encoder and tests are
  updated.
- **Acceptance measurement:** before/after
  `scripts/metrics/sdk_codesizes_rv32.sh` over the example contracts;
  confirm `_contract_abort_encoded::<ContractExn<LibraryExn>>` and
  `SyscallExn::write_xdr` symbols are gone from counter's ELF
  (`llvm-nm --size-sort`).

## Alternatives considered

- **Per-field `encode_into` only, no structural hand-rolling** (i.e.
  keep the derive): captures none of the win; the derive's inlined
  cascades and plumbing are the cost.
- **Hand-written varint put helpers in the SDK:** ~200–300 B smaller
  than the `encode_into` approach in theory, but duplicates the CXS
  integer spec in a second implementation and the shared primitives are
  already linked by other encode paths — rejected in favor of reusing
  `encode_into`.
- **Exposing xdrs's `pub(crate) Put` trait / a slice writer:** cleanest
  layering but requires an xdrs release + workspace pin bump; not
  needed since `encode_into` suffices.
- **Const-precomputed abort byte strings for constant variants:**
  maximal win for e.g. `nnint_overflow`, but `syscall_throw` and the
  sized variants carry runtime data so a runtime encoder is needed
  anyway; extra complexity for marginal gain.

## Risks

- The size win depends on LLVM not re-inlining the shared primitives
  into the new encoder's arms; the acceptance measurement is the
  arbiter. If the measured win is disappointing, the hand-written-puts
  variant is the documented fallback.
- Wire divergence risk concentrates in the direct discriminant byte
  stores; the exhaustive byte-equality tests are the guard.
