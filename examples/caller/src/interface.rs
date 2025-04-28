use erc20::interface::Amount;
use sdk::prelude::*;

#[contract(client = "client")]
pub trait ICaller {
    fn initialize(this: Option<&mut Self>, token_contract: &ContractAddress);
    fn check_balance(&self, account: &ContractAddress) -> Amount;
}
