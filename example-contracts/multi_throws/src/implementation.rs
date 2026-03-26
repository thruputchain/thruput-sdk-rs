use super::*;
use sdk::*;

#[derive(Default)]
pub struct Contract;

/// An error type NOT declared in throws — will end up in Unknown
#[derive(Debug, PartialEq, thiserror::Error, ReadXDR, WriteXDR, Exn)]
#[error("secret error")]
pub struct SecretError;

#[contract]
impl MultiThrows for Contract {
    fn succeed() -> u32 {
        42
    }

    fn throw_alpha_not_found() {
        abort(&AlphaError::NotFound);
    }

    fn throw_alpha_overflow() {
        abort(&AlphaError::Overflow);
    }

    fn throw_beta_denied() {
        abort(&BetaError::Denied);
    }

    fn throw_undeclared() {
        abort(&SecretError);
    }
}
