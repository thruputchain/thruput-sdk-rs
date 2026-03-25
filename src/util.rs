pub fn ensure<E>(pred: bool, err: E)
where
    E: crate::xdr::WriteXDR,
{
    if !pred {
        crate::abort(&err)
    }
}
