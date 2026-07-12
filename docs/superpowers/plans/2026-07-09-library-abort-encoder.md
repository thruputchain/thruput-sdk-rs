# Hand-Rolled `LibraryExn` Abort Encoder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the derived `encode_scratch::<ContractExn<LibraryExn>>` abort-encode path with a small hand-rolled encoder, removing ~1.3–1.6 KB of `.text` from every contract.

**Architecture:** A new `crates/sdk/src/exn/abort_encode.rs` module encodes `[u64 selector][i32 discriminant][payload]` directly into a fixed stack `MUBuf`, using the existing public `xdrs::cxs::encode_into` for every variable-width field (no new varint code). The three existing `ContractExn<LibraryExn>` encode sites (`library_throw`, `client_library_throw`, `exception_encoding_error`) reroute to it. Wire format is pinned byte-for-byte against the derived encoder by exhaustive tests.

**Tech Stack:** Rust (no_std sdk crate), xdrs CXS serialization, rv32 example-contract builds for size measurement.

**Spec:** `docs/superpowers/specs/2026-07-09-library-abort-encoder-design.md` — read it before starting any task.

## Global Constraints

- No new CXS varint implementation: all variable-width integers go through `xdrs::cxs::encode_into`. The ONLY format knowledge the encoder owns is "a non-negative enum discriminant ≤ 30 is its own single CXS header byte".
- Wire output must be byte-identical to `encode_xdr(&ContractExn::new(exn))` in both cfg modes (`debugging` on/off).
- The derived `WriteXDR`/`ReadXDR` impls on `LibraryExn` and its payload types must NOT be removed or changed (clients/native-exec/spec tooling use them; they are the test oracle).
- User-exception path (`throw<E>`, `_contract_abort_encoded`, `encode_scratch`) stays untouched except where `exception_encoding_error`'s body is replaced.
- Work on branch `geoff/library-abort-encoder` (repo hook blocks commits on `main`).
- `cargo fmt` before every commit; code must pass `cargo clippy -p sdk`.

---

### Task 1: `abort_encode` module with wire-compat tests

**Files:**
- Create: `crates/sdk/src/exn/abort_encode.rs`
- Modify: `crates/sdk/src/exn/mod.rs` (add `pub(crate) mod abort_encode;` after line 24's `pub(crate) mod cross_exn;`)
- Modify: `crates/sdk/src/exn/library.rs` (add `errno()` accessor to `impl SyscallExn`, around line 237 next to `callno()`)
- Test: inline `#[cfg(test)] mod tests` in `crates/sdk/src/exn/abort_encode.rs` (the function is `pub(crate)`, so tests must be in-module, not in `tests/`)

**Interfaces:**
- Consumes: `crate::internal::MUBuf<N>` (`new()`, `clear()`, `spare_capacity_mut() -> &mut [MaybeUninit<u8>]` — note: returns the WHOLE backing array regardless of `len`, so index it at your own cursor — `unsafe set_len(usize)`, `as_ref() -> &[u8]`); `xdrs::cxs::encode_into<T: WriteXDR + ?Sized>(&T, &mut [MaybeUninit<u8>]) -> WriteResult<usize>`; `crate::internal::encode_xdr` (test oracle, alloc-only — available in tests since dev features imply std/alloc); `crate::exn::cross_exn::ContractExn::new` (test oracle); `crate::trap()`.
- Produces: `pub(crate) const ABORT_MAX: usize` and `pub(crate) fn encode_library_abort(exn: &LibraryExn, buf: &mut MUBuf<ABORT_MAX>)` (encodes and `set_len`s the buffer; caller reads `buf.as_ref()`); `SyscallExn::errno(&self) -> SysErr` (`pub(crate)`). Task 2 relies on these exact names.

- [ ] **Step 1: Add the `errno` accessor and module skeleton**

In `crates/sdk/src/exn/library.rs`, inside `impl SyscallExn` (after `callno()`):

```rust
    pub(crate) fn errno(&self) -> SysErr {
        self.errno
    }
```

In `crates/sdk/src/exn/mod.rs`, after `pub(crate) mod cross_exn;`:

```rust
pub(crate) mod abort_encode;
```

Create `crates/sdk/src/exn/abort_encode.rs`:

```rust
//! Hand-rolled encoder for the SDK's own abort payload,
//! `ContractExn<LibraryExn>`.
//!
//! Every contract can abort with a `LibraryExn` (storage getters,
//! syscall wrappers, `re_throw`, `none_if_missing`), so the generic
//! `encode_scratch::<ContractExn<LibraryExn>>` monomorphization was the
//! single largest function in a minimal contract (~2.2 KB of rv32
//! `.text`). `LibraryExn`'s encoding is small and statically bounded, so
//! this module encodes it directly into a fixed stack buffer instead:
//! `[u64 selector][discriminant][payload]`, with every variable-width
//! field delegated to the shared `xdrs::cxs::encode_into` primitives.
//!
//! Byte-for-byte wire compatibility with the derived encoder is pinned
//! by the exhaustive tests below; the derived `WriteXDR` impl remains
//! the oracle (and the client-side decoder's counterpart).

use super::library::LibraryExn;
use super::wrapper::{AuthTokenError, CallingError, HandlingError};
use crate::internal::MUBuf;
use crate::xdr::WriteXDR;
use core::mem::MaybeUninit;

/// Fixed encode-buffer size, covering the worst-case variant:
/// selector (u64 varint, ≤ 9) + discriminant (1) + payload.
/// Release worst case is `StorageIndexOutOfBounds { u64, u64 }`:
/// 9 + 1 + 9 + 9 = 28 (`FaultString` is a ZST). With `debugging`,
/// `FaultString` is `Option<heapless::String<500>>` on the wire —
/// bool (1) + length (≤ 3) + 500 bytes — so the worst case is
/// 9 + 1 + 1 + 504 = 515. The `max_width_variants_fit` test pins this.
#[cfg(not(feature = "debugging"))]
pub(crate) const ABORT_MAX: usize = 32;
#[cfg(feature = "debugging")]
pub(crate) const ABORT_MAX: usize = 544;

/// Encode one field at offset `*n` through the shared `encode_into`
/// primitives. Statically infallible: `ABORT_MAX` covers the worst-case
/// encoding of every variant (see the sizing test), so the trap arm is
/// unreachable.
fn put<T: WriteXDR + ?Sized>(
    buf: &mut MUBuf<ABORT_MAX>,
    n: &mut usize,
    x: &T,
) {
    match xdrs::cxs::encode_into(x, &mut buf.spare_capacity_mut()[*n..]) {
        Ok(written) => *n += written,
        Err(_) => crate::trap(),
    }
}

/// A non-negative XDR enum discriminant `<= 30` encodes as its own
/// single CXS header byte (6-bit value field, sign/size bits clear).
/// This is the only piece of CXS format knowledge this module owns;
/// the wire-compat tests pin it against the derived encoder.
fn put_disc(buf: &mut MUBuf<ABORT_MAX>, n: &mut usize, d: u8) {
    debug_assert!(d <= 30);
    buf.spare_capacity_mut()[*n] = MaybeUninit::new(d);
    *n += 1;
}

/// CXS-encode `ContractExn<LibraryExn>` — `[selector][discriminant]
/// [payload]` — into `buf`, replacing its contents.
pub(crate) fn encode_library_abort(
    exn: &LibraryExn,
    buf: &mut MUBuf<ABORT_MAX>,
) {
    use crate::exn::Exn;

    buf.clear();
    let mut n = 0;
    put(buf, &mut n, &LibraryExn::SELECTOR);
    match exn {
        LibraryExn::Syscall(se) => {
            put_disc(buf, &mut n, 0);
            put(buf, &mut n, &se.callno());
            put(buf, &mut n, &se.errno());
        }
        LibraryExn::Handling(h) => {
            put_disc(buf, &mut n, 1);
            match h {
                HandlingError::DecodingArgs(fs) => {
                    put_disc(buf, &mut n, 0);
                    put(buf, &mut n, fs);
                }
                HandlingError::EncodingSuccess(fs) => {
                    put_disc(buf, &mut n, 1);
                    put(buf, &mut n, fs);
                }
                HandlingError::EncodingException(fs) => {
                    put_disc(buf, &mut n, 2);
                    put(buf, &mut n, fs);
                }
                HandlingError::EncodingEvent(fs) => {
                    put_disc(buf, &mut n, 3);
                    put(buf, &mut n, fs);
                }
            }
        }
        LibraryExn::Calling(c) => {
            put_disc(buf, &mut n, 2);
            match c {
                CallingError::Invalid => put_disc(buf, &mut n, 0),
                CallingError::DecodingSuccess(fs) => {
                    put_disc(buf, &mut n, 1);
                    put(buf, &mut n, fs);
                }
                CallingError::DecodingException(fs) => {
                    put_disc(buf, &mut n, 2);
                    put(buf, &mut n, fs);
                }
                CallingError::EncodingArgs(fs) => {
                    put_disc(buf, &mut n, 3);
                    put(buf, &mut n, fs);
                }
            }
        }
        LibraryExn::Called(fs) => {
            put_disc(buf, &mut n, 3);
            put(buf, &mut n, fs);
        }
        LibraryExn::AuthToken(a) => {
            put_disc(buf, &mut n, 4);
            match a {
                AuthTokenError::MintEncodeFailure(fs) => {
                    put_disc(buf, &mut n, 0);
                    put(buf, &mut n, fs);
                }
                AuthTokenError::ConsumeDecodeFailure(fs) => {
                    put_disc(buf, &mut n, 1);
                    put(buf, &mut n, fs);
                }
                AuthTokenError::ReceiveAuthHandlesFailure(fs) => {
                    put_disc(buf, &mut n, 2);
                    put(buf, &mut n, fs);
                }
            }
        }
        LibraryExn::BytestringWrongSize { expected, found } => {
            put_disc(buf, &mut n, 5);
            put(buf, &mut n, expected);
            put(buf, &mut n, found);
        }
        LibraryExn::NonNegativeIntUnderflow => put_disc(buf, &mut n, 6),
        LibraryExn::NonNegativeIntOverflow => put_disc(buf, &mut n, 7),
        LibraryExn::StorageSerialization => put_disc(buf, &mut n, 8),
        LibraryExn::StorageIndexOutOfBounds { len, access } => {
            put_disc(buf, &mut n, 9);
            put(buf, &mut n, len);
            put(buf, &mut n, access);
        }
        LibraryExn::StorageKeypathOverflow => put_disc(buf, &mut n, 10),
        LibraryExn::RetrocausalQueueEntryWrongSize { expected, found } => {
            put_disc(buf, &mut n, 11);
            put(buf, &mut n, expected);
            put(buf, &mut n, found);
        }
    }
    // Safety: bytes 0..n were initialized contiguously — `put` via
    // `encode_into` (which fills its slice from index 0) at offset `n`,
    // `put_disc` by direct store.
    unsafe { buf.set_len(n) };
}
```

Notes for the implementer:
- Field order in struct variants is declaration order (`expected` then `found`; `len` then `access`) — that is what the derive writes.
- `LibraryExn` is `#[non_exhaustive]`, but that does not relax same-crate matches: the match above is checked exhaustive, which is the intended growth tripwire.
- If the compiler reports privacy errors on `se.callno()` / `se.errno()`: `callno()` already exists (public); `errno()` is the accessor you added in this step.

- [ ] **Step 2: Write the wire-compat tests (failing first)**

Append to `crates/sdk/src/exn/abort_encode.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::exn::cross_exn::ContractExn;
    use crate::exn::{FaultString, SyscallExn};
    use crate::internal::encode_xdr;
    use crate::sys::{SysErr, SyscallNo};

    /// Assert the hand-rolled encoding is byte-identical to the derived
    /// encoder (the wire oracle).
    fn assert_wire_compat(exn: LibraryExn) {
        let mut buf = MUBuf::<ABORT_MAX>::new();
        encode_library_abort(&exn, &mut buf);
        let oracle = encode_xdr(&ContractExn::new(exn)).unwrap();
        assert_eq!(buf.as_ref(), oracle.as_ref());
    }

    fn fault_strings() -> [FaultString; 3] {
        let long = core::str::from_utf8(&[b'x'; 500]).unwrap();
        [
            FaultString::empty(),
            FaultString::from_display("boom"),
            FaultString::from_display(long),
        ]
    }

    const U64_EDGES: &[u64] = &[
        0,
        1,
        30,
        31,
        61,
        62,
        63,
        64,
        127,
        128,
        255,
        256,
        65535,
        65536,
        u32::MAX as u64 - 1,
        u32::MAX as u64,
        u32::MAX as u64 + 1,
        u64::MAX,
    ];

    const U32_EDGES: &[u32] =
        &[0, 1, 30, 31, 62, 63, 127, 128, 255, 256, 65535, 65536, u32::MAX];

    const I32_EDGES: &[i32] = &[
        i32::MIN,
        -65537,
        -65536,
        -32769,
        -32768,
        -129,
        -128,
        -33,
        -32,
        -31,
        -1,
        0,
        1,
        30,
        31,
        62,
        63,
        127,
        128,
        32767,
        32768,
        i32::MAX,
    ];

    #[test]
    fn no_payload_variants_wire_compat() {
        assert_wire_compat(LibraryExn::NonNegativeIntUnderflow);
        assert_wire_compat(LibraryExn::NonNegativeIntOverflow);
        assert_wire_compat(LibraryExn::StorageSerialization);
        assert_wire_compat(LibraryExn::StorageKeypathOverflow);
    }

    #[test]
    fn syscall_variant_wire_compat_across_int_widths() {
        for &callno in I32_EDGES {
            for &errno in I32_EDGES {
                assert_wire_compat(LibraryExn::Syscall(SyscallExn::new(
                    SyscallNo::from(callno),
                    SysErr(errno),
                )));
            }
        }
    }

    #[test]
    fn faultstring_variants_wire_compat() {
        for fs in fault_strings() {
            assert_wire_compat(LibraryExn::Handling(
                HandlingError::DecodingArgs(fs.clone()),
            ));
            assert_wire_compat(LibraryExn::Handling(
                HandlingError::EncodingSuccess(fs.clone()),
            ));
            assert_wire_compat(LibraryExn::Handling(
                HandlingError::EncodingException(fs.clone()),
            ));
            assert_wire_compat(LibraryExn::Handling(
                HandlingError::EncodingEvent(fs.clone()),
            ));
            assert_wire_compat(LibraryExn::Calling(
                CallingError::DecodingSuccess(fs.clone()),
            ));
            assert_wire_compat(LibraryExn::Calling(
                CallingError::DecodingException(fs.clone()),
            ));
            assert_wire_compat(LibraryExn::Calling(
                CallingError::EncodingArgs(fs.clone()),
            ));
            assert_wire_compat(LibraryExn::Called(fs.clone()));
            assert_wire_compat(LibraryExn::AuthToken(
                AuthTokenError::MintEncodeFailure(fs.clone()),
            ));
            assert_wire_compat(LibraryExn::AuthToken(
                AuthTokenError::ConsumeDecodeFailure(fs.clone()),
            ));
            assert_wire_compat(LibraryExn::AuthToken(
                AuthTokenError::ReceiveAuthHandlesFailure(fs),
            ));
        }
        assert_wire_compat(LibraryExn::Calling(CallingError::Invalid));
    }

    #[test]
    fn sized_variants_wire_compat_across_int_widths() {
        for &expected in U32_EDGES {
            for &found in U32_EDGES {
                assert_wire_compat(LibraryExn::BytestringWrongSize {
                    expected,
                    found,
                });
                assert_wire_compat(
                    LibraryExn::RetrocausalQueueEntryWrongSize {
                        expected,
                        found,
                    },
                );
            }
        }
        for &len in U64_EDGES {
            for &access in U64_EDGES {
                assert_wire_compat(LibraryExn::StorageIndexOutOfBounds {
                    len,
                    access,
                });
            }
        }
    }

    /// The widest possible instance of every payload-carrying variant
    /// fits ABORT_MAX — this is what makes the trap arm in `put`
    /// statically unreachable.
    #[test]
    fn max_width_variants_fit() {
        let long = core::str::from_utf8(&[b'x'; 500]).unwrap();
        let cases = [
            LibraryExn::Syscall(SyscallExn::new(
                SyscallNo::from(i32::MIN),
                SysErr(i32::MIN),
            )),
            LibraryExn::Handling(HandlingError::EncodingException(
                FaultString::from_display(long),
            )),
            LibraryExn::Called(FaultString::from_display(long)),
            LibraryExn::StorageIndexOutOfBounds {
                len: u64::MAX,
                access: u64::MAX,
            },
            LibraryExn::BytestringWrongSize {
                expected: u32::MAX,
                found: u32::MAX,
            },
        ];
        for exn in cases {
            let mut buf = MUBuf::<ABORT_MAX>::new();
            encode_library_abort(&exn, &mut buf);
            assert!(buf.as_ref().len() <= ABORT_MAX);
        }
    }
}
```

- [ ] **Step 3: Run the tests, verify they pass (or debug to green)**

Run from repo root:

```
cargo test -p sdk abort_encode
```

Expected: all 5 tests PASS. These tests are written against the completed implementation from Step 1, so they should pass immediately; any failure is a real wire divergence — debug the encoder against the oracle bytes (print both slices), do NOT adjust the oracle. Likely divergence causes: wrong nested discriminant value, wrong field order, or `SyscallNo`'s generated `WriteXDR` not being a plain i32 (the oracle comparison resolves this automatically since `put(&se.callno())` delegates to the same impl — a failure here means something structural).

Note: this test build runs with `dev` features (the sdk dev-dependency self-injects them), so `debugging` is ON: `ABORT_MAX` is 544 and `FaultString` carries real strings. The release arm differs only by `FaultString`'s own ZST `WriteXDR` impl (`#[inline(always)] Ok(())`), which has its own roundtrip tests in `exn/fault.rs`.

- [ ] **Step 4: Sanity-negative check (mutation test of the test)**

Temporarily change one discriminant in the encoder (e.g. `put_disc(buf, &mut n, 7)` → `6` in the `NonNegativeIntOverflow` arm), re-run `cargo test -p sdk abort_encode`, and confirm `no_payload_variants_wire_compat` FAILS. Revert the mutation and re-run to green. This proves the oracle comparison actually bites.

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt
cargo clippy -p sdk 2>&1 | tail -5   # expect no new warnings
git add crates/sdk/src/exn/abort_encode.rs crates/sdk/src/exn/mod.rs crates/sdk/src/exn/library.rs
git commit -m "sdk: hand-rolled ContractExn<LibraryExn> abort encoder with wire-compat tests"
```

---

### Task 2: Abort entry points and rerouting

**Files:**
- Modify: `crates/sdk/src/exn/abort_encode.rs` (append entry points)
- Modify: `crates/sdk/src/exn/library.rs:167-179` (`library_throw`, `client_library_throw`)
- Modify: `crates/sdk/src/contract_op.rs:112-133` (`exception_encoding_error`)

**Interfaces:**
- Consumes: `encode_library_abort(&LibraryExn, &mut MUBuf<ABORT_MAX>)` and `ABORT_MAX` from Task 1; `crate::contract_op::{contract_abort, abort_breadcrumb}`; `crate::internal::MUBuf`.
- Produces: `pub(crate) fn library_abort(exn: &LibraryExn) -> !` and `pub(crate) fn client_library_abort(exn: &LibraryExn) -> !` in `crate::exn::abort_encode`.

- [ ] **Step 1: Add the entry points**

Append to `crates/sdk/src/exn/abort_encode.rs` (above the `#[cfg(test)]` module):

```rust
/// Abort the frame with a library exception, via the hand-rolled
/// encoder. This is the terminal step of `library_throw` (and therefore
/// of `re_throw`, `none_if_missing`, and every `syscall_throw`-family
/// helper): no contract monomorphizes the generic encode machinery for
/// the SDK's own abort path.
#[cfg_attr(feature = "debugging", track_caller)]
pub(crate) fn library_abort(exn: &LibraryExn) -> ! {
    #[cfg(all(feature = "debugging", feature = "std"))]
    crate::contract_op::abort_breadcrumb("Throwing", "Location", exn);

    let mut buf = MUBuf::<ABORT_MAX>::new();
    encode_library_abort(exn, &mut buf);
    crate::contract_abort(buf.as_ref())
}

/// `library_abort` for client-side codegen; differs only in the
/// breadcrumb's location label (mirrors the old
/// `client_contract_abort_encoded` split).
#[cfg_attr(feature = "debugging", track_caller)]
pub(crate) fn client_library_abort(exn: &LibraryExn) -> ! {
    #[cfg(all(feature = "debugging", feature = "std"))]
    crate::contract_op::abort_breadcrumb("Throwing", "Interface location", exn);

    let mut buf = MUBuf::<ABORT_MAX>::new();
    encode_library_abort(exn, &mut buf);
    crate::contract_abort(buf.as_ref())
}
```

If `crate::contract_abort` is not resolvable at that path, use `crate::contract_op::contract_abort` (the function is `pub fn contract_abort(buf: &[u8]) -> !` in `contract_op.rs:33`).

- [ ] **Step 2: Reroute `library_throw` / `client_library_throw`**

In `crates/sdk/src/exn/library.rs`, replace the two function bodies (lines 167–179):

```rust
#[cfg_attr(feature = "debugging", track_caller)]
pub fn library_throw<E: Into<LibraryExn> + core::fmt::Debug>(e: E) -> ! {
    super::abort_encode::library_abort(&e.into())
}

#[cfg_attr(feature = "debugging", track_caller)]
pub fn client_library_throw<E: Into<LibraryExn> + core::fmt::Debug>(
    e: E,
) -> ! {
    super::abort_encode::client_library_abort(&e.into())
}
```

Keep the existing doc comments. The `use crate::exn::cross_exn::ContractExn`-style imports these bodies previously needed may now be unused in this file — remove any the compiler flags.

- [ ] **Step 3: Reroute `exception_encoding_error`**

In `crates/sdk/src/contract_op.rs`, replace `exception_encoding_error` (lines 112–133) with:

```rust
// A user exception failed to encode in `_contract_abort_encoded`:
// report it as a typed library exception instead of trapping. Reachable
// only from the generic `throw<E>` path — library exceptions go through
// the hand-rolled `abort_encode` encoder, which is infallible (fixed
// buffer sized to the worst-case variant) and does not touch the shared
// scratch, so no second-failure fallback is needed here anymore.
#[cold]
#[inline(never)]
fn exception_encoding_error(err: WriteError) -> ! {
    use crate::exn::LibraryExn;

    crate::exn::abort_encode::library_abort(&LibraryExn::from(
        HandlingError::EncodingException(FaultString::via_display(&err)),
    ))
}
```

(`HandlingError` and `FaultString` are already imported at the top of `contract_op.rs`. The old body's `ContractExn` import and `crate::trap()` fallback go away with it.)

- [ ] **Step 4: Run the full sdk test suite**

```
cargo test -p sdk
```

Expected: PASS (this exercises the mock executor's throw→decode round trips, storage getter re-throws, and the compile-fail suite). If a test asserts on abort *bytes* and fails, that is a wire divergence Task 1's tests missed — stop and investigate; do not adjust the failing test.

- [ ] **Step 5: Run the workspace build + clippy to catch cfg/import fallout**

```
cargo check --workspace
cargo clippy -p sdk 2>&1 | tail -5
```

Expected: clean. (`native-exec` and `macros` consume the rerouted functions.)

- [ ] **Step 6: Format and commit**

```bash
cargo fmt
git add crates/sdk/src/exn/abort_encode.rs crates/sdk/src/exn/library.rs crates/sdk/src/contract_op.rs
git commit -m "sdk: route all LibraryExn aborts through the hand-rolled encoder"
```

---

### Task 3: Size measurement and rv32 verification

**Files:** none modified (measurement only; results go in the PR description). Save intermediate output under the session scratchpad, not /tmp.

**Interfaces:**
- Consumes: the committed Tasks 1–2; `scripts/dev.sh` helpers (`eg_build_rv32_all` — builds every `example-contracts/*-rv32` wrapper with `cargo build -Zbuild-std=core,alloc -p <pkg> -F entrypoints --release --target riscv32im-unknown-none-elf` and runs the dispatch + strings checks); `scripts/metrics/sdk_codesizes_rv32.sh <elf-dir> <contracts-dir>`.
- Produces: a before/after size table and symbol-absence proof for the PR body.

- [ ] **Step 1: Build baseline (main) rv32 contracts in a worktree**

```bash
git worktree add "$SCRATCHPAD/baseline-main" main
cd "$SCRATCHPAD/baseline-main"
bash -c 'source scripts/dev.sh && eg_build_rv32_all'
scripts/metrics/sdk_codesizes_rv32.sh target/riscv32im-unknown-none-elf/release example-contracts | tee "$SCRATCHPAD/sizes-before.txt"
```

(`$SCRATCHPAD` is the session scratchpad directory. If `eg_build_rv32_all` is slow, building just `counter-rv32` plus two or three others and running the metrics script on those is acceptable — but use the same subset for the after-measurement.)

- [ ] **Step 2: Build branch rv32 contracts and measure**

```bash
cd <repo root>
bash -c 'source scripts/dev.sh && eg_build_rv32_all'
scripts/metrics/sdk_codesizes_rv32.sh target/riscv32im-unknown-none-elf/release example-contracts | tee "$SCRATCHPAD/sizes-after.txt"
diff "$SCRATCHPAD/sizes-before.txt" "$SCRATCHPAD/sizes-after.txt"
```

Expected: `text=` drops by roughly 1.3–1.6 KB for counter (and comparable amounts elsewhere); `eg_build_rv32_all`'s built-in dispatch and strings checks pass on every contract.

- [ ] **Step 3: Confirm the monomorphizations are gone**

```bash
/opt/homebrew/opt/llvm/bin/llvm-nm --size-sort --demangle target/riscv32im-unknown-none-elf/release/counter_rv32 | grep -i "abort_encoded\|SyscallExn"
```

Expected: NO `_contract_abort_encoded::<...ContractExn<...LibraryExn>>` symbol and no `SyscallExn ... write_xdr` symbol. (A `library_abort`/`encode_library_abort` symbol of a few hundred bytes is the replacement and is expected.)

If the size win is materially below ~1 KB on counter, check `llvm-nm --size-sort` for what LLVM re-inlined; the documented fallback (spec "Risks") is replacing per-field `encode_into` calls with local varint puts — do NOT do that unilaterally, report the numbers and stop for review.

- [ ] **Step 4: Clean up the worktree**

```bash
git worktree remove "$SCRATCHPAD/baseline-main" --force
```

Keep `sizes-before.txt` / `sizes-after.txt` for the PR body.

---

### Task 4: PR and CI

- [ ] **Step 1: Push and open the PR**

```bash
git push -u origin geoff/library-abort-encoder
gh pr create --title "sdk: hand-rolled ContractExn<LibraryExn> abort encoder" --body "<summary: problem (largest fn in every contract, ~22% of counter .text), design (spec link: docs/superpowers/specs/2026-07-09-library-abort-encoder-design.md), wire-compat test strategy, and the before/after size table from Task 3>"
```

- [ ] **Step 2: Monitor CI to green**

Use the `check-ci` skill: monitor PR checks, fix failures, address Greptile review comments. Known pre-existing flake: `erc20_low_level` in full-suite runs (documented in project memory) — rerun rather than chase if it's the only failure.
