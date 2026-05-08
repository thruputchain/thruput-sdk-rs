#![cfg_attr(not(any(feature = "dev", feature = "spec")), no_std)]

use groundhog_erc20_if::*;
use sdk::*;

/// A bearer-permit to spend an amount of some token.
///
/// For now, we blindly trust that this has been authorized by `owner`.
/// TODO: Sign it.
///
#[contracttype]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct Permit {
    pub owner: ContractAddress,
    pub token: ContractAddress,
    pub amount: Amount,
}

impl Permit {
    pub fn new(
        owner: &ContractAddress,
        token: &ContractAddress,
        amount: Amount,
    ) -> Self {
        Self {
            owner: *owner,
            token: *token,
            amount,
        }
    }
}

#[sdk::contract]
pub trait ERC20Permit {
    /// Transfer from `owner` to `receiver` address, using allowance
    fn transfer_with_permit(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
        permit: Permit,
    );
}
