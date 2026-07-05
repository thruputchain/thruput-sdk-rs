use sdk::*;

#[contract_exception]
pub enum AlphaError {
    NotFound = 1,
    Overflow = 2,
}

#[contract_exception]
pub enum BetaError {
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

    /// Throws an error that is not decodable by the client.
    fn throw_undecodable();

    /// Throws malicious library error
    fn throw_malicious_library_error();

    /// Calls `throw_undecodable()` on `target` via the generated client,
    /// catches the resulting `BadDecode`, performs an unrelated SDK op
    /// (log emit — this reuses the call scratch), then re-throws.
    /// Pins the guarantee that a `CaughtException` survives arbitrary
    /// scratch reuse and re-throws verbatim.
    fn relay_undecodable(target: ContractAddress);
}
