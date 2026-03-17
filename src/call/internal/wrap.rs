use crate::exn::{CrossExn, Exn};
use crate::{AbortInfo, Never};

/// Handle an abort thrown during a method call
///
pub fn method_abort_handler<'a, E>(info: AbortInfo<'a>) -> Never
where
    E: Exn + 'static,
{
    // Downcast the payload by reference and serialize directly,
    // without cloning.
    CrossExn::<E>::serialize_abort(info.payload());
}
