//! Example caller of ERC20 contract

#![cfg_attr(not(feature = "mock"), no_std)]

use erc20_if::ERC20Client;
use erc20_if::Amount;
use sdk::storage_prelude::*;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Caller {
    token_contract: ContractAddress,
}

#[contract(client = "CallerClient")]
impl Caller {
    pub fn initialize(&mut self, token_contract: &ContractAddress) {
        self.token_contract = token_contract.clone();
    }

    pub fn check_balance(&self, account: &ContractAddress) -> Amount {
        let mut token = ERC20Client::new(&self.token_contract);

        token.get_balance_of(account).unwrap()
    }
}
