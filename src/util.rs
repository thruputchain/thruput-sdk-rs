use crate::exn::*;

pub fn ensure<E>(pred: bool, err: E)
where
    E: Exn + 'static + core::fmt::Display,
{
    if !pred {
        crate::abort(&err)
    }
}
