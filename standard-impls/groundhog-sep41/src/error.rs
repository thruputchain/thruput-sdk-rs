#[sdk::contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum SEP41Error {
    #[cfg_attr(feature = "dev", error("Amount must be non-negative"))]
    NegativeAmount = 1,
    #[cfg_attr(feature = "dev", error("Approval would already be expired"))]
    InvalidExpiration = 2,
    #[cfg_attr(feature = "dev", error("Token metadata is not set"))]
    MetadataUnavailable = 3,
}
