pub fn ensure<E>(pred: bool, err: E)
where
    E: crate::Exn,
{
    if !pred {
        crate::throw(err)
    }
}
