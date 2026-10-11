use crate::Amount;
use sdk::ensure;

// The native token precompile uses this fixed wire identity.
#[sdk::contract_exception(wire_crate = "groundhog-erc20-if")]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum ERC20Error {
    #[cfg_attr(feature = "dev", error("Contract already initialized"))]
    AlreadyInitialized = 1,

    #[cfg_attr(feature = "dev", error("Caller must be contract owner"))]
    MustBeContractOwner = 2,

    #[cfg_attr(feature = "dev", error("Provided amount too large"))]
    TooLarge = 3,

    #[cfg_attr(feature = "dev", error("Balance must be zero before deleting"))]
    BalanceNotZero = 4,

    #[cfg_attr(feature = "dev", error("Amount must be non-negative"))]
    NegativeAmount = 5,

    #[cfg_attr(feature = "dev", error("Internal error"))]
    Internal = 100,
}

/// Return `amount`, throwing [`ERC20Error::NegativeAmount`] if it is negative.
pub fn check_amount(amount: Amount) -> Amount {
    ensure(amount >= 0, ERC20Error::NegativeAmount);
    amount
}

#[cfg(test)]
mod wire_identity_tests {
    #[test]
    fn selector_matches_the_native_token_precompile() {
        assert_eq!(
            <super::ERC20Error as sdk::exn::Exn>::SELECTOR,
            0xb19947acb2fa14ff
        );
    }
}
