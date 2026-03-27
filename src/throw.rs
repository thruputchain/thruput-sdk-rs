#[cfg_attr(feature = "debugging", track_caller)]
pub fn throw<E>(exn: E) -> !
where
    E: crate::Exn,
{
    let exn = crate::exn::cross_exn::ContractExn::new(exn);
    crate::contract_abort_encoded(&exn)
}
