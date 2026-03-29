use crate::HasherClient;
pub use bottle_test_if::*;
use sdk::*;

#[derive(Debug)]
#[storage]
pub struct BT {
    target_addr: Option<ContractAddress>,
}

#[contract(exports = "bottle_exports", version = "0.1.0")]
impl BottleTest for BT {
    #[init]
    fn setup(&mut self, target_contract_hash: &SHA256Hash) {
        let nonce: SHA256Hash = [7; 32].into();
        let target_addr = deploy_contract(&nonce, target_contract_hash);

        self.target_addr = Some(target_addr);
    }

    fn run(&mut self) {
        let target_addr = self.target_addr.as_ref().unwrap_or_else(||
            panic!("bottle-test: target_addr unset; please run setup() before run()"));

        let client = HasherClient::new(target_addr);
        client.run();
    }
}
