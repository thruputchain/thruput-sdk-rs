use bottle_test_if::BottleTest;
use sdk::storage::Value;
use sdk::*;
use versioned_interface::AdderClient;

#[storage]
pub struct Tester {
    target: Value<Option<ContractAddress>>,
}

#[contract]
impl BottleTest for Tester {
    fn setup(&mut self, target_contract_hash: &SHA256Hash) {
        let nonce: SHA256Hash = [7; 32].into();
        let address = deploy_contract(&nonce, target_contract_hash);
        self.target.set(&Some(address));
    }

    fn run(&mut self) {
        let address = self.target.get().unwrap();
        let client = AdderClient::new(&address);
        assert!(client.add_one(41) == 42);
    }
}
