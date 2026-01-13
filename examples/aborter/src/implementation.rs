use sdk::*;

#[derive(Default)]
struct Contract;

#[contract(client = ContractClient, storage = "false")]
impl Contract {
    pub fn initialize(&mut self, contract_owner: &ContractAddress) {
        let key = StorageKey::default();

        // Store ourselves
        bytestring_set(&key, contract_owner.as_ref()).unwrap();
    }

    pub fn abort_contract(&mut self) {
        abort(&"Hello");
    }
}
