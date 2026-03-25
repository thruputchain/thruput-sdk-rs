#[cfg_attr(feature = "debugging", track_caller)]
pub fn throw<E>(exn: E) -> !
where
    E: crate::Exn
{
    let exn = crate::contract_exn(exn);

    crate::contract_abort_encoded(&exn)
}
