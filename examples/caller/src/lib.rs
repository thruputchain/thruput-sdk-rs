//! Example caller of ERC20 contract

#![no_std]

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
    fn initialize(this: Option<&mut Self>, token_contract: &ContractAddress) {

        // Make sure we haven't already been initialized
        if this.is_some() {
            panic!("Contract already initialized");
        }

        // Create a default storage value, pass it to
        // the given closure for initialization, and
        // store it at the root key.
        initialize_root(|storage: &mut Self| {
            storage.token_contract = token_contract.clone()
        }).unwrap();
    }

    fn check_balance(&self, account: &ContractAddress) -> Amount {
        let token = ERC20Client::new(&self.token_contract);

        token.get_balance_of(account).unwrap()
    }
}
