//! Example caller of ERC20 contract

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

pub use thruput_erc20_if as erc20_if;

use erc20_if::Amount;
use erc20_if::ERC20Client;
use thruput_sdk::storage::Value;
use thruput_sdk::*;

#[storage]
pub struct Caller {
    token_contract: Value<ContractAddress>,
}

#[contract(client = CallerClient)]
impl Caller {
    pub fn initialize(&mut self, token_contract: &ContractAddress) {
        self.token_contract.set(token_contract);
    }

    pub fn check_balance(&self, account: &ContractAddress) -> Amount {
        let token = ERC20Client::new(&self.token_contract.get());

        token.get_balance_of(account)
    }
}
