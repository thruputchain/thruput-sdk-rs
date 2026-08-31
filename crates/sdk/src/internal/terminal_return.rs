//! Terminal-return plumbing for generated entrypoint wrappers.
//!
//! These fns diverge via the Return syscall on real guests: **`Drop` does
//! not run past them.** A live deferred-write guard (e.g.
//! [`ValueMut`](crate::storage::ValueMut)) at the call would have its
//! flush silently discarded on-chain — while the native mock implements
//! Return as an unwind, so destructors DO run in tests, hiding the loss.
//! That asymmetry is why these are not public API: generated wrappers call
//! them only after the user method (and all its locals) has returned.
//! If you call them directly, drop all storage guards first.

use crate::sys;
use crate::xdr::*;
use crate::{CollectAuthHandles, FaultString, HandlingError, library_throw};

#[doc(hidden)]
pub fn contract_return(buf: &[u8], auth_token_ids: &[u32]) -> ! {
    crate::storage::live_guards::assert_no_live_guards();

    let revert = false;

    let auth_token_ids_bytes =
        crate::call::auth_handles_as_bytes(auth_token_ids);

    sys::raw_Return(buf, revert, auth_token_ids_bytes)
}

#[doc(hidden)]
#[inline]
#[cfg_attr(not(target_arch = "riscv32"), track_caller)]
pub fn contract_return_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized + CollectAuthHandles,
{
    // Collect auth handles first (own them), then encode the return value
    // into the reused scratch in place and hand it straight to the syscall —
    // no fresh 4 KiB buffer on the (small) guest stack.
    let auth_handles = crate::call::collect_auth_handle_bytes(ret)
        .unwrap_or_else(|e| {
            // This is the successful-return path, so a failure here is an
            // `EncodingSuccess` condition — not `EncodingException`, which is
            // reserved for a failed *abort* encode.
            library_throw(HandlingError::EncodingSuccess(
                FaultString::from_display(e),
            ));
        });

    crate::call::with_scratch(|buf| {
        crate::internal::encode_scratch(buf, ret)
            .unwrap_or_else(|e| crate::exn::encoding_success_throw(e));
        contract_return(buf.as_ref(), auth_handles.as_slice());
    })
}
