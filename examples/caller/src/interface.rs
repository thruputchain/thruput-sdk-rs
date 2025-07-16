use erc20_if::Amount;
use sdk::prelude::*;

#[contract(client = "client")]
pub trait ICaller {
    fn initialize(&mut self, token_contract: &ContractAddress);
    fn check_balance(&self, account: &ContractAddress) -> Amount;
}
