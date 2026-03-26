pub fn ensure<E>(pred: bool, err: E)
where
    E: crate::exn::Exn,
{
    if !pred {
        crate::abort(&err)
    }
}
