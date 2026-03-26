use sdk::*;
use thiserror::Error;

#[derive(WriteXDR, ReadXDR, Copy, Clone, Debug, PartialEq, Error, Exn)]
#[repr(i32)]
pub enum AlphaError {
    #[error("alpha: not found")]
    NotFound = 1,
    #[error("alpha: overflow")]
    Overflow = 2,
}

#[derive(WriteXDR, ReadXDR, Copy, Clone, Debug, PartialEq, Error, Exn)]
#[repr(i32)]
pub enum BetaError {
    #[error("beta: denied")]
    Denied = 1,
}

#[contract(client = MultiClient, throws(AlphaError, BetaError))]
pub trait MultiThrows {
    fn succeed() -> u32;
    fn throw_alpha_not_found();
    fn throw_alpha_overflow();
    fn throw_beta_denied();
    fn throw_undeclared();
}
