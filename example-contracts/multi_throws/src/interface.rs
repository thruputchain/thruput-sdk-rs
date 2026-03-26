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
}
