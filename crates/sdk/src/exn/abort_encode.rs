//! Hand-rolled encoder for the SDK's own abort payload,
//! `ContractExn<LibraryExn>`.
//!
//! Every contract can abort with a `LibraryExn` (storage getters,
//! syscall wrappers, `re_throw`, `none_if_missing`), so the generic
//! `encode_scratch::<ContractExn<LibraryExn>>` monomorphization was the
//! single largest function in a minimal contract (~2.2 KB of rv32
//! `.text`). `LibraryExn`'s encoding is small and statically bounded, so
//! this module encodes it directly into a fixed stack buffer instead:
//! `[u64 selector][discriminant][payload]`.
//!
//! Integer fields use the local `put_u64`/`put_i32` helpers, which own
//! a direct copy of the CXS integer rule for the abort path; per-field
//! `encode_into` measured at ~1.2 KB per contract (writer setup +
//! bounds/Result plumbing per call site) — the very cost this module
//! exists to remove. `FaultString` still delegates to `encode_into` so
//! both cfg modes stay wire-correct by construction.
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

// CXS integer encoding constants (mirrors xdrs/src/cxs/put.rs and limits.rs).
// Values ≤ UH_MAX (62) encode as a single header byte; 63 is the multibyte
// marker. Bits 6-7 of the marker encode the payload width (S_* below).
const V_MULTIBYTE: u8 = 63;
const S_1BYTE: u8 = 0 << 6;
const S_2BYTE: u8 = 1 << 6;
const S_4BYTE: u8 = 2 << 6;
const S_8BYTE: u8 = 3 << 6;

/// Write `bytes` into `buf` at offset `n`, returning the new offset.
fn put_bytes(buf: &mut MUBuf<ABORT_MAX>, n: usize, bytes: &[u8]) -> usize {
    let spare = &mut buf.spare_capacity_mut()[n..n + bytes.len()];
    for (dst, &src) in spare.iter_mut().zip(bytes) {
        *dst = MaybeUninit::new(src);
    }
    n + bytes.len()
}

/// CXS unsigned integer. Also used for `u32` fields: the CXS encoding
/// depends only on the value, so widening is lossless.
fn put_u64(buf: &mut MUBuf<ABORT_MAX>, n: usize, v: u64) -> usize {
    if v <= 62 {
        put_bytes(buf, n, &[v as u8])
    } else if v <= u8::MAX as u64 {
        put_bytes(buf, n, &[V_MULTIBYTE | S_1BYTE, v as u8])
    } else if v <= u16::MAX as u64 {
        let b = (v as u16).to_le_bytes();
        put_bytes(buf, n, &[V_MULTIBYTE | S_2BYTE, b[0], b[1]])
    } else if v <= u32::MAX as u64 {
        let b = (v as u32).to_le_bytes();
        put_bytes(buf, n, &[V_MULTIBYTE | S_4BYTE, b[0], b[1], b[2], b[3]])
    } else {
        let b = v.to_le_bytes();
        put_bytes(
            buf,
            n,
            &[
                V_MULTIBYTE | S_8BYTE,
                b[0],
                b[1],
                b[2],
                b[3],
                b[4],
                b[5],
                b[6],
                b[7],
            ],
        )
    }
}

/// CXS signed integer (`SysErr` is the only signed abort field). Small
/// values use the 6-bit one's-complement header-byte form.
fn put_i32(buf: &mut MUBuf<ABORT_MAX>, n: usize, v: i32) -> usize {
    if (-31..=31).contains(&v) {
        let nu = v.unsigned_abs() as u8;
        let h = if v >= 0 {
            nu
        } else {
            (!nu & 0b01_1111) | 0b0010_0000
        };
        put_bytes(buf, n, &[h])
    } else if (i8::MIN as i32..=i8::MAX as i32).contains(&v) {
        put_bytes(buf, n, &[V_MULTIBYTE | S_1BYTE, v as u8])
    } else if (i16::MIN as i32..=i16::MAX as i32).contains(&v) {
        let b = (v as i16).to_le_bytes();
        put_bytes(buf, n, &[V_MULTIBYTE | S_2BYTE, b[0], b[1]])
    } else {
        let b = v.to_le_bytes();
        put_bytes(buf, n, &[V_MULTIBYTE | S_4BYTE, b[0], b[1], b[2], b[3]])
    }
}

/// Encode one `FaultString` field at offset `n` through `encode_into`.
/// Used only for `FaultString`, whose encoding is cfg-dependent; both
/// cfg modes stay wire-correct by construction via the shared primitive.
/// Statically infallible: `ABORT_MAX` covers the worst-case encoding of
/// every variant (see the sizing test), so the trap arm is unreachable.
fn put<T: WriteXDR + ?Sized>(
    buf: &mut MUBuf<ABORT_MAX>,
    n: usize,
    x: &T,
) -> usize {
    match xdrs::cxs::encode_into(x, &mut buf.spare_capacity_mut()[n..]) {
        Ok(written) => n + written,
        Err(_) => crate::trap(),
    }
}

/// A non-negative XDR enum discriminant `<= 30` encodes as its own
/// single CXS header byte (6-bit value field, sign/size bits clear).
/// This is the only piece of CXS format knowledge this module owns;
/// the wire-compat tests pin it against the derived encoder.
fn put_disc(buf: &mut MUBuf<ABORT_MAX>, n: usize, d: u8) -> usize {
    debug_assert!(d <= 30);
    buf.spare_capacity_mut()[n] = MaybeUninit::new(d);
    n + 1
}

/// CXS-encode `ContractExn<LibraryExn>` — `[selector][discriminant]
/// [payload]` — into `buf`, replacing its contents.
pub(crate) fn encode_library_abort(
    exn: &LibraryExn,
    buf: &mut MUBuf<ABORT_MAX>,
) {
    use crate::exn::Exn;

    buf.clear();
    let mut n = put_u64(buf, 0, LibraryExn::SELECTOR);
    match exn {
        LibraryExn::Syscall(se) => {
            n = put_disc(buf, n, 0);
            n = put_u64(buf, n, se.callno().0 as u64);
            n = put_i32(buf, n, se.errno().0);
        }
        LibraryExn::Handling(h) => {
            n = put_disc(buf, n, 1);
            match h {
                HandlingError::DecodingArgs(fs) => {
                    n = put_disc(buf, n, 0);
                    n = put(buf, n, fs);
                }
                HandlingError::EncodingSuccess(fs) => {
                    n = put_disc(buf, n, 1);
                    n = put(buf, n, fs);
                }
                HandlingError::EncodingException(fs) => {
                    n = put_disc(buf, n, 2);
                    n = put(buf, n, fs);
                }
                HandlingError::EncodingEvent(fs) => {
                    n = put_disc(buf, n, 3);
                    n = put(buf, n, fs);
                }
            }
        }
        LibraryExn::Calling(c) => {
            n = put_disc(buf, n, 2);
            match c {
                CallingError::Invalid => {
                    n = put_disc(buf, n, 0);
                }
                CallingError::DecodingSuccess(fs) => {
                    n = put_disc(buf, n, 1);
                    n = put(buf, n, fs);
                }
                CallingError::DecodingException(fs) => {
                    n = put_disc(buf, n, 2);
                    n = put(buf, n, fs);
                }
                CallingError::EncodingArgs(fs) => {
                    n = put_disc(buf, n, 3);
                    n = put(buf, n, fs);
                }
            }
        }
        LibraryExn::Called(fs) => {
            n = put_disc(buf, n, 3);
            n = put(buf, n, fs);
        }
        LibraryExn::AuthToken(a) => {
            n = put_disc(buf, n, 4);
            match a {
                AuthTokenError::MintEncodeFailure(fs) => {
                    n = put_disc(buf, n, 0);
                    n = put(buf, n, fs);
                }
                AuthTokenError::ConsumeDecodeFailure(fs) => {
                    n = put_disc(buf, n, 1);
                    n = put(buf, n, fs);
                }
                AuthTokenError::ReceiveAuthHandlesFailure(fs) => {
                    n = put_disc(buf, n, 2);
                    n = put(buf, n, fs);
                }
            }
        }
        LibraryExn::BytestringWrongSize { expected, found } => {
            n = put_disc(buf, n, 5);
            n = put_u64(buf, n, *expected as u64);
            n = put_u64(buf, n, *found as u64);
        }
        LibraryExn::NonNegativeIntUnderflow => {
            n = put_disc(buf, n, 6);
        }
        LibraryExn::NonNegativeIntOverflow => {
            n = put_disc(buf, n, 7);
        }
        LibraryExn::StorageSerialization => {
            n = put_disc(buf, n, 8);
        }
        LibraryExn::StorageIndexOutOfBounds { len, access } => {
            n = put_disc(buf, n, 9);
            n = put_u64(buf, n, *len);
            n = put_u64(buf, n, *access);
        }
        LibraryExn::StorageKeypathOverflow => {
            n = put_disc(buf, n, 10);
        }
        LibraryExn::RetrocausalQueueEntryWrongSize { expected, found } => {
            n = put_disc(buf, n, 11);
            n = put_u64(buf, n, *expected as u64);
            n = put_u64(buf, n, *found as u64);
        }
    }
    // Safety: bytes 0..n were initialized contiguously — `put` via
    // `encode_into` (which fills its slice from index 0) at offset `n`,
    // `put_disc` and `put_u64`/`put_i32` by direct store.
    unsafe { buf.set_len(n) };
}

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
    crate::contract_op::contract_abort(buf.as_ref())
}

/// `library_abort` for client-side codegen; differs only in the
/// breadcrumb's location label (mirrors the old
/// `client_contract_abort_encoded` split).
#[cfg_attr(feature = "debugging", track_caller)]
pub(crate) fn client_library_abort(exn: &LibraryExn) -> ! {
    #[cfg(all(feature = "debugging", feature = "std"))]
    crate::contract_op::abort_breadcrumb(
        "Throwing",
        "Interface location",
        exn,
    );

    let mut buf = MUBuf::<ABORT_MAX>::new();
    encode_library_abort(exn, &mut buf);
    crate::contract_op::contract_abort(buf.as_ref())
}

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
        assert_eq!(buf.as_ref(), &oracle[..]);
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

    const U32_EDGES: &[u32] = &[
        0,
        1,
        30,
        31,
        62,
        63,
        127,
        128,
        255,
        256,
        65535,
        65536,
        u32::MAX,
    ];

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
