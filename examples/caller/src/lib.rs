//! Example caller of ERC20 contract

#![cfg_attr(not(feature = "mock"), no_std)]

extern crate alloc;

pub mod interface;

use erc20::interface::client::ERC20Client;
use erc20::interface::{Amount, ERC20};
use sdk::storage_prelude::*;
use crate::interface::ICaller;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Caller {
    token_contract: ContractAddress,
}

#[contract]
impl ICaller for Caller {
    fn initialize(&mut self, token_contract: &ContractAddress) {
        self.token_contract = token_contract.clone();
    }

    fn check_balance(&self, account: &ContractAddress) -> Amount {
        let token = ERC20Client::new(&self.token_contract);

        token.get_balance_of(account).unwrap()
    }
}
