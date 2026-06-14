#![allow(unexpected_cfgs)]

use sdk::*;

#[sdk::storage]
pub struct Helper;

#[contract]
impl Helper {
    pub fn ping(&self) -> u32 {
        self.seven()
    }

    fn seven(&self) -> u32 {
        7
    }
}

fn main() {
    let _client = HelperClient::new(&ContractAddress::default());
}
