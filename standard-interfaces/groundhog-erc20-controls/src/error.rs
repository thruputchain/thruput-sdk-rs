#[sdk::contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum ERC20ControlsError {
    #[cfg_attr(feature = "dev", error("Token is not initialized"))]
    NotInitialized = 1,
    #[cfg_attr(feature = "dev", error("Caller is not authorized"))]
    Unauthorized = 2,
    #[cfg_attr(feature = "dev", error("Holder is not authorized"))]
    HolderNotAuthorized = 3,
    #[cfg_attr(
        feature = "dev",
        error("Operation is disabled by token policy")
    )]
    OperationDisabled = 4,
    #[cfg_attr(feature = "dev", error("Clawback requires revocation"))]
    InvalidPolicy = 5,
    #[cfg_attr(feature = "dev", error("Amount must be non-negative"))]
    NegativeAmount = 6,
    #[cfg_attr(feature = "dev", error("Approval would already be expired"))]
    InvalidExpiration = 7,
    #[cfg_attr(feature = "dev", error("Token metadata is not set"))]
    MetadataUnavailable = 8,
}
