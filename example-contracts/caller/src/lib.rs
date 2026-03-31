//! Example caller of ERC20 contract

#![cfg_attr(not(feature = "dev"), no_std)]

pub use groundhog_erc20_if as erc20_if;

use erc20_if::Amount;
use erc20_if::ERC20Client;
use sdk::storage::Value;
use sdk::*;

#[storage]
pub struct Caller {
    token_contract: Value<ContractAddress>,
}

#[contract(client = CallerClient)]
impl Caller {
    #[init]
    pub fn initialize(&mut self, token_contract: &ContractAddress) {
        self.token_contract.set(token_contract);
    }

    pub fn check_balance(&self, account: &ContractAddress) -> Amount {
        let token = ERC20Client::new(&self.token_contract.get());

        token.get_balance_of(account)
    }
}
