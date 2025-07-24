// Only to be compiled for native_exec

use sdk::prelude::*;
use erc20_if::ERC20Client;

#[derive(Default)]
pub struct Bencher;

#[contract(client = BencherClient, storage = "false")]
impl Bencher {
    pub fn test_erc20(&self, erc20_addr: &ContractAddress) {
        let mut erc20 = ERC20Client::new(erc20_addr);

        let owner = get_current_address().unwrap();

        erc20.initialize(&owner).unwrap();

        let _ = erc20.get_balance().unwrap();
    }
}
