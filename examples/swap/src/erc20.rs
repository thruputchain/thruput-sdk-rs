#![allow(unused_variables)]

use crate::interfaces::ierc20::IERC20;
use sdk::prelude::*;

pub struct ERC20Client<'a>(pub &'a ContractAddress);

impl IERC20 for ERC20Client<'_> {
    fn transfer(&self, to: &ContractAddress, amount: u128) -> bool {
        todo!()
    }

    fn balance_of(&self, address: &ContractAddress) -> u128 {
        todo!()
    }
}
