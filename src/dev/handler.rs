use crate::exn::{CrossExn, Exn};
use crate::{AbortInfo, Never};

/// Handle an abort thrown during a method call (test/dev version)
///
pub fn test_abort_handler<'a, E>(info: AbortInfo<'a>) -> Never
where
    E: Exn + Clone + 'static,
{
    // In test builds we construct an owned CrossExn so it can be
    // inspected with Debug/Display before aborting.
    let a = CrossExn::<E>::from_abort_info(info);

    crate::contract_abort_encoded(&a);
}
