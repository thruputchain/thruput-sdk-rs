use crate::interfaces::ierc20::IERC20;
use sdk::prelude::*;

pub struct ERC20<'a>(pub &'a mut ContractAddress);

impl IERC20 for ERC20<'_> {
    fn transfer(&mut self, to: ContractAddress, amount: u128) -> bool {
        todo!()
    }
}
