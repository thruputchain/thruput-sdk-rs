use super::*;
use sdk::*;

#[derive(Default)]
pub struct Contract;

/// An error type NOT declared in throws — will end up in Unknown
#[derive(Debug, PartialEq, thiserror::Error, ReadXDR, WriteXDR)]
#[exn]
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

    fn forward_alpha(target: ContractAddress) {
        // Non-try call: if the target aborts, re_abort forwards the error
        MultiClient::new(&target).throw_alpha_not_found();
    }

    fn forward_undeclared(target: ContractAddress) {
        // Non-try call: SecretError isn't in throws, forwarded as raw bytes
        MultiClient::new(&target).throw_undeclared();
    }
}
