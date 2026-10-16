use adder_v1::AdderClient as AdderV1Client;
use adder_v2::AdderClient as AdderV2Client;
use bottle_test_if::BottleTest;
use thruput_sdk::storage::Value;
use thruput_sdk::*;

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
        let v1 = AdderV1Client::new(&address);
        let v2 = AdderV2Client::new(&address);
        assert!(v1.add_one(41) == 42);
        let large = u64::from(u32::MAX) + 41;
        assert!(v2.add_one(large) == large + 1);
        assert!(v1.add_one(99) == 100);
    }
}
