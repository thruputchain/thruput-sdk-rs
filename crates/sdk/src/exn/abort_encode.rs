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
