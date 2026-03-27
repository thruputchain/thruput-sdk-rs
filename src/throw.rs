#[cfg_attr(feature = "debugging", track_caller)]
pub fn throw<E>(exn: E) -> !
where
    E: crate::Exn,
{
    crate::exn::cross_exn::serialize_error(&exn)
}
