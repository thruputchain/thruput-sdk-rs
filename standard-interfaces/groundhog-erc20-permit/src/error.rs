/// Why a permit was refused by the token contract.
///
/// Signature, nonce and expiry failures are not here: those are decided by
/// whatever minted the grant, before the token contract is involved. Nor is
/// "wrong token": the runtime refuses to consume a grant anywhere but at its
/// named consumer.
#[sdk::contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum ERC20PermitError {
    /// `transfer_from_signed` was called by someone other than the spender the
    /// owner authorized.
    #[cfg_attr(feature = "dev", error("Caller is not the authorized spender"))]
    NotSpender = 1,
    /// The grant authorizes a different operation.
    #[cfg_attr(
        feature = "dev",
        error("Grant authorizes a different operation")
    )]
    WrongAction = 2,
}
