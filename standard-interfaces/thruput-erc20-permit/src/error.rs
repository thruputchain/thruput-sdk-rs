/// Why a permit was refused by the token contract.
///
/// Signature, nonce and expiry failures are not here: those are decided by
/// whatever minted the grant, before the token contract is involved. Nor is
/// "wrong token": the runtime refuses to consume a grant anywhere but at its
/// named consumer.
// The native token precompile uses this fixed wire identity.
#[sdk::contract_exception(wire_crate = "groundhog-erc20-permit-if")]
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

#[cfg(test)]
mod wire_identity_tests {
    #[test]
    fn selector_matches_the_native_token_precompile() {
        assert_eq!(
            <super::ERC20PermitError as sdk::exn::Exn>::SELECTOR,
            0xe8abb072f5171148
        );
    }
}
