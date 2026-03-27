use sdk::*;

#[contract_error]
pub enum AlphaError {
    #[error("alpha: not found")]
    NotFound = 1,
    #[error("alpha: overflow")]
    Overflow = 2,
}

#[contract_error]
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

    /// Calls `throw_alpha_not_found()` on another instance via the
    /// non-try method. The re_throw path should forward the error.
    fn forward_alpha(target: ContractAddress);

    /// Calls `throw_undeclared()` on another instance via the non-try
    /// method. The forwarded SecretError should arrive as Unknown.
    fn forward_undeclared(target: ContractAddress);

    /// Panics intentionally.
    fn panics();

    /// Calls `panics()` on another instance via the non-try method.
    /// The fault should propagate as a fault.
    fn forward_fault(target: ContractAddress);
}
