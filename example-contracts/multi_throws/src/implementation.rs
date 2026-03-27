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
        throw(AlphaError::NotFound);
    }

    fn throw_alpha_overflow() {
        throw(AlphaError::Overflow);
    }

    fn throw_beta_denied() {
        throw(BetaError::Denied);
    }

    fn throw_undeclared() {
        throw(SecretError);
    }

    fn forward_alpha(target: ContractAddress) {
        // Non-try call: if the target throws, re_throw forwards the error
        MultiClient::new(&target).throw_alpha_not_found();
    }

    fn forward_undeclared(target: ContractAddress) {
        // Non-try call: SecretError isn't in throws, forwarded as raw bytes
        MultiClient::new(&target).throw_undeclared();
    }

    fn panics() {
        panic!("intentional panic");
    }

    fn forward_fault(target: ContractAddress) {
        // Non-try call: the inner panic should propagate as a fault
        MultiClient::new(&target).panics();
    }
}
