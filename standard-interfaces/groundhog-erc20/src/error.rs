use sdk::throw;

pub type Result<T, E = ERC20Error> = core::result::Result<T, E>;

/*
#[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, thiserror::Error)]
#[sdk::exn]
#[repr(i32)]
#[non_exhaustive]
*/
#[sdk::contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum ERC20Error {
    #[cfg_attr(feature = "dev", error("Contract already initialized"))]
    AlreadyInitialized = 1,

    #[cfg_attr(feature = "dev", error("Caller must be contract owner"))]
    MustBeContractOwner = 2,

    #[cfg_attr(feature = "dev", error("Provided amount too large"))]
    TooLarge = 3,

    #[cfg_attr(feature = "dev", error("Internal error"))]
    Internal = 100,
}

pub fn amount_to_diff(n: u128) -> i128 {
    n.try_into().unwrap_or_else(|_| throw(ERC20Error::TooLarge))
}
