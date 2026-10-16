#[thruput_sdk::contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum AccessControlError {
    #[cfg_attr(feature = "dev", error("Caller is missing the required role"))]
    MissingRole = 1,
}
