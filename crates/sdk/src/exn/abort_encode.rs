//! Encoder for the SDK's own abort payload, `ContractExn<LibraryExn>`.
//!
//! Every contract can abort with a `LibraryExn` (storage getters,
//! syscall wrappers, `re_throw`, `none_if_missing`), so this path is
//! kept off the generic fallible encode machinery, whose
//! `encode_scratch::<ContractExn<LibraryExn>>` monomorphization was the
//! single largest function in a minimal contract (~1.5 KB of rv32
//! `.text`, post-ICF). `LibraryExn`'s encoding is statically bounded,
//! so it goes through xdrs's unchecked write path instead: infallible
//! by sizing (`ABORT_MAX`), no `WriteResult` plumbing, no fallback
//! buffers, and no per-contract monomorphization of the fallible
//! machinery — the derived `write_xdr_unchecked` does the encoding,
//! this module only sizes the buffer.
//!
//! Byte-for-byte wire compatibility with the checked derived encoder is
//! pinned by the exhaustive tests below; the derived `WriteXDR` impl
//! remains the oracle (and the client-side decoder's counterpart).

use super::library::LibraryExn;
use crate::internal::MUBuf;
use crate::xdr::WriteXDR;
use xdrs::cxs::unchecked_buf_writer::UncheckedBufWriter;

/// Fixed encode-buffer size: the `ContractExn` wrapper's u64 selector
/// at worst-case width plus `LibraryExn`'s derived worst case
/// (`#[derive(ConstXDRSize)]`). `max_width_variants_fit` pins that
/// every real encoding fits.
pub(crate) const ABORT_MAX: usize = {
    let payload = match <LibraryExn as xdrs::cxs::ConstMaxSize>::MAX_SIZE {
        Some(n) => n,
        None => panic!("LibraryExn wire encoding must be bounded"),
    };
    xdrs::cxs::const_writer::size_u64(u64::MAX) + payload
};

// `ABORT_MAX` sizes a stack buffer in `library_abort`, so it must stay small
// -- and, more importantly, must not depend on how the crate was built. It
// was 544 while `FaultString` serialized its message under `debugging`, which
// both cost the guest stack and made a contract's abort payload differ
// between a host test and the shipped artifact. Tripping this means a variant
// started carrying diagnostic text onto the wire again; keep the text
// host-side (see `exn::fault`) rather than raising the bound.
const _: () = assert!(
    ABORT_MAX < 64,
    "ABORT_MAX grew: a LibraryExn variant is carrying a payload it should be \
     keeping host-side"
);

/// CXS-encode `ContractExn<LibraryExn>` — `[selector][discriminant]
/// [payload]` — into `buf`, replacing its contents.
pub(crate) fn encode_library_abort(
    exn: &LibraryExn,
    buf: &mut MUBuf<ABORT_MAX>,
) {
    // By-reference: `ContractExn<&LibraryExn>` — same selector and wire
    // bytes as by-value (see the `Exn for &T` blanket impl), no clone.
    let ce = crate::exn::cross_exn::ContractExn::new(exn);
    buf.clear();
    let mut w = UncheckedBufWriter::new(buf.spare_capacity_mut());
    // Safety: ABORT_MAX covers the worst-case encoding of every variant
    // (pinned by `max_width_variants_fit`), and the exception's contents
    // respect their declared bounds by construction.
    unsafe {
        ce.write_xdr_unchecked(&mut w);
    }
    let n = w.written();
    unsafe { buf.set_len(n) };
}

/// Abort the frame with a library exception, via the hand-rolled
/// encoder. This is the terminal step of `library_throw` (and therefore
/// of `re_throw`, `none_if_missing`, and every `syscall_throw`-family
/// helper): no contract monomorphizes the generic encode machinery for
/// the SDK's own abort path.
///
/// `inline(never)` pins the single-copy property against LTO, exactly
/// like the old `_contract_abort_encoded`: every throw-helper call site
/// must reach this one copy of the encoder by direct call.
#[inline(never)]
#[cfg_attr(feature = "debugging", track_caller)]
pub(crate) fn library_abort(exn: &LibraryExn) -> ! {
    #[cfg(all(feature = "debugging", feature = "std"))]
    crate::contract_op::abort_breadcrumb("Throwing", "Location", exn);

    let mut buf = MUBuf::<ABORT_MAX>::new();
    encode_library_abort(exn, &mut buf);
    crate::contract_op::contract_abort(buf.as_ref())
}

/// `library_abort` for client-side codegen; differs only in the
/// breadcrumb's location label. `inline(never)`: see
/// `library_abort` (in release the two are identical and the linker's
/// ICF folds them).
#[inline(never)]
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

// These tests cover the shipped encoding. `FaultString` has one wire form
// on every target and feature combination -- empty -- so `ABORT_MAX` and
// every assertion below are the same here as in a guest build, and the
// integer and discriminant rules were already cfg-independent. (Before
// `FaultString` carried its message host-side, sdk's dev-dependency on
// itself forced `debugging` on for every test build, and the encoding
// that actually shipped was asserted by construction rather than run.)
#[cfg(test)]
mod tests {
    use super::*;
    use crate::exn::cross_exn::ContractExn;
    use crate::exn::{
        AuthTokenError, CallingError, FaultString, HandlingError, SyscallExn,
    };
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
        assert_wire_compat(LibraryExn::CapacityConstraintAlreadyWritten);
        assert_wire_compat(LibraryExn::UnknownMethod);
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
