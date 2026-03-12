//! Example caller of ERC20 contract

#![cfg_attr(not(feature = "dev"), no_std)]

pub use groundhog_erc20_if as erc20_if;

use erc20_if::Amount;
use erc20_if::ERC20Client;
use sdk::*;

#[storage]
pub struct Caller {
    token_contract: ContractAddress,
}

#[contract(client = CallerClient)]
impl Caller {
    #[init]
    pub fn initialize(&mut self, token_contract: &ContractAddress) {
        self.token_contract = *token_contract
    }

    pub fn check_balance(&self, account: &ContractAddress) -> Amount {
        let mut token = ERC20Client::new(&self.token_contract);

        token.get_balance_of(account)
    }
}
