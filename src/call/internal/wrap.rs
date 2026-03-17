use crate::exn::{CrossExn, Exn};
use crate::{AbortInfo, Never};

/// Handle an abort thrown during a method call
///
/// In production builds, serializes the payload by reference (no clone).
/// In debugging builds, constructs a full CrossExn with location/backtrace.
///
#[cfg(not(feature = "debugging"))]
pub fn method_abort_handler<'a, E>(info: AbortInfo<'a>) -> Never
where
    E: Exn + 'static,
{
    CrossExn::<E>::serialize_abort(info.payload());
}

#[cfg(feature = "debugging")]
pub fn method_abort_handler<'a, E>(info: AbortInfo<'a>) -> Never
where
    E: Exn + Clone + 'static,
{
    let a = CrossExn::<E>::from_abort_info(info);
    crate::contract_abort_encoded(&a);
}
