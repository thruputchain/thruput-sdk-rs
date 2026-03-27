use sdk::{ReadXDR, WriteXDR, throw};
use thiserror::Error;

pub type Result<T, E = ERC20Error> = core::result::Result<T, E>;

#[cfg(feature = "spec")]
use xdrs::XDR;

/*
#[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, thiserror::Error)]
#[sdk::exn]
#[repr(i32)]
#[non_exhaustive]
*/
#[sdk::contract_error]
#[non_exhaustive]
#[cfg_attr(feature = "spec", derive(XDR))]
pub enum ERC20Error {
    #[error("Contract already initialized")]
    AlreadyInitialized = 1,

    #[error("Caller must be contract owner")]
    MustBeContractOwner = 2,

    #[error("Provided amount too large")]
    TooLarge = 3,

    #[error("Internal error")]
    Internal = 100,
}

pub fn amount_to_diff(n: u128) -> i128 {
    n.try_into().unwrap_or_else(|_| throw(ERC20Error::TooLarge))
}
