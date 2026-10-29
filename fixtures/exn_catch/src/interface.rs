use thruput_sdk::*;

#[contract_exception]
pub enum AlphaError {
    NotFound = 1,
    Overflow = 2,
}

#[contract_exception]
pub enum BetaError {
    Denied = 1,
}

/// Thrown, but deliberately absent from the interface's `throws` list — the
/// extension-error shape that motivated matching on selectors. A caller
/// matches it as a peer of the declared errors, with no separate step.
#[contract_exception]
pub struct Undeclared;

#[contract(client = ExnCatchClient, throws(AlphaError, BetaError))]
pub trait ExnCatch {
    fn succeed() -> u32;

    fn throw_alpha_not_found();
    fn throw_alpha_overflow();
    fn throw_beta_denied();

    /// Throws [`Undeclared`], which `throws` does not mention.
    fn throw_undeclared();

    /// Throws one of the SDK's own exceptions.
    fn throw_library();

    /// Call one of the above on `target` and report what came back:
    /// 0 success, 1 fault, 2/3 `AlphaError`, 4 `BetaError`, 5
    /// `Undeclared`, 6 `LibraryExn`, 7 anything else.
    ///
    /// Exercises owned exception dispatch in guest code.
    fn classify(target: ContractAddress, which: u32) -> u32;

    /// The same classification using a borrowed exception mapper.
    fn classify_mapped(target: ContractAddress, which: u32) -> u32;

    /// Classify with the mapper, then encode a new typed rejection.
    fn classify_then_throw(target: ContractAddress, which: u32);

    /// Forward an undeclared exception through the borrowed mapper unchanged.
    fn rethrow(target: ContractAddress);

    fn fault();
    fn malformed_abort();
}
