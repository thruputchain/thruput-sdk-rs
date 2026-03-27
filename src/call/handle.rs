//! Handling contract calls

use crate::exn::{FaultString, HandlingError, library_throw};
use crate::xdr::*;

pub fn return_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized,
{
    let return_buf = crate::call::internal::contract_call_encode(ret)
        .unwrap_or_else(|e| {
            library_throw(HandlingError::EncodingSuccess(
                FaultString::via_display(&e),
            ))
        });

    crate::contract_return(return_buf.as_ref());
}

pub fn abort_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized,
{
    let return_buf = crate::call::internal::contract_call_encode(ret)
        .unwrap_or_else(|e| {
            library_throw(HandlingError::EncodingException(
                FaultString::via_display(&e),
            ))
        });

    crate::contract_abort(return_buf.as_ref());
}
