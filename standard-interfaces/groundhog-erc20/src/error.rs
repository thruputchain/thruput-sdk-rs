use sdk::{ReadXDR, WriteXDR, abort};
use thiserror::Error;

pub type Result<T, E = ERC20Error> = core::result::Result<T, E>;

#[cfg(feature = "spec")]
use xdrs::XDR;

#[derive(Error, Debug, PartialEq, Clone, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "spec", derive(XDR))]
#[non_exhaustive]
#[repr(i32)]
pub enum ERC20Error {
    /* Not used?
    #[error("None")]
    None = 0,
    */
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
    n.try_into()
        .unwrap_or_else(|_| abort(&ERC20Error::TooLarge))
}
