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
    /// that provably reuses the call scratch (`Hash::hash`, which goes
    /// through `with_scratch`), then re-throws. Pins the guarantee that a
    /// `RawException` survives arbitrary scratch reuse and re-throws
    /// verbatim, in both builds (each owns an independent copy of the wire
    /// bytes — heap under `alloc`, an inline `SCRATCH_MAX` buffer in the
    /// no-alloc guest).
    fn relay_undecodable(target: ContractAddress);

    /// Calls `throw_undecodable()` on `target` twice, holding both
    /// resulting `BadDecode` catches alive at the same time (with a
    /// scratch-reusing op interleaved between them), then re-throws the
    /// first. Pins that catching is unlimited and concurrent in the
    /// no-alloc guest too: no shared per-instance slot, no "at most one
    /// live" restriction — identical semantics to `alloc`.
    fn relay_undecodable_twice(target: ContractAddress);
}
