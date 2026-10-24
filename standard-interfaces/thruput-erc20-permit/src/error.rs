/// Why a permit was refused by the token contract.
///
/// Signature, nonce and expiry failures are not here: those are decided by
/// whatever minted the grant, before the token contract is involved. Nor is
/// "wrong token": the runtime refuses to consume a grant anywhere but at its
/// named consumer.
#[thruput_sdk::contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum ERC20PermitError {
    /// `transfer_from_signed` was called by someone other than the spender the
    /// owner authorized.
    #[cfg_attr(feature = "dev", error("Caller is not the authorized spender"))]
    NotSpender = 1,

    /// `transfer_from_signed` was asked to move more than the grant allows.
    #[cfg_attr(
        feature = "dev",
        error("Requested amount exceeds the signed grant")
    )]
    ExceedsGrant = 2,
}
