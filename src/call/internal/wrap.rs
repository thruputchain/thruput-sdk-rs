use crate::{AbortInfo, Never};
use crate::exn::{Exn, CrossExn};

/// Handle an abort thrown during a method call
///
pub fn method_abort_handler<'a, E>(info: AbortInfo<'a>) -> Never
where
    E: Exn + Clone + 'static
{
    // Downcast to a concrete exception
    let a = CrossExn::<E>::from_abort_info(info);

    // Encode that and call VM abort
    crate::contract_abort_encoded(&a);
}
