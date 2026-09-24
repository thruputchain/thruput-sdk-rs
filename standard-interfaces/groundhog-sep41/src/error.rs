#[sdk::contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum TokenError {
    #[cfg_attr(feature = "dev", error("Token is not initialized"))]
    NotInitialized = 1,
    #[cfg_attr(feature = "dev", error("Token is already initialized"))]
    AlreadyInitialized = 2,
    #[cfg_attr(
        feature = "dev",
        error("Caller is not authorized for this operation")
    )]
    Unauthorized = 3,
    #[cfg_attr(feature = "dev", error("Amount must be non-negative"))]
    NegativeAmount = 4,
    #[cfg_attr(feature = "dev", error("Insufficient balance"))]
    InsufficientBalance = 5,
    #[cfg_attr(feature = "dev", error("Insufficient allowance"))]
    InsufficientAllowance = 6,
    #[cfg_attr(feature = "dev", error("Allowance expiry is in the past"))]
    InvalidExpiry = 7,
    #[cfg_attr(feature = "dev", error("Holder is not authorized"))]
    HolderNotAuthorized = 8,
    #[cfg_attr(
        feature = "dev",
        error("Operation is disabled by token policy")
    )]
    OperationDisabled = 9,
    #[cfg_attr(feature = "dev", error("Clawback requires revocation"))]
    InvalidPolicy = 10,
    #[cfg_attr(
        feature = "dev",
        error("Amount exceeds the token supply limit")
    )]
    Overflow = 11,
}
