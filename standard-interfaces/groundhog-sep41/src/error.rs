#[sdk::contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum SEP41Error {
    #[cfg_attr(
        feature = "dev",
        error("Caller does not match the authorized address")
    )]
    Unauthorized = 1,
    #[cfg_attr(feature = "dev", error("Amount must be non-negative"))]
    NegativeAmount = 2,
    #[cfg_attr(
        feature = "dev",
        error("Nonzero approval expires before the current block")
    )]
    InvalidExpiration = 3,
    #[cfg_attr(feature = "dev", error("Token metadata is unavailable"))]
    MetadataUnavailable = 4,
}
