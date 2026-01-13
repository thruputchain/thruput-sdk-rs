use sdk::*;

#[derive(Default)]
struct Contract;

#[contract(client = ContractClient, storage = "false")]
impl Contract {
    pub fn initialize(&self, contract_owner: &ContractAddress) {
        let key = StorageKey::default();

        // Store ourselves
        bytestring_set(&key, contract_owner.as_ref()).unwrap();
    }

    pub fn abort_contract(&self) {
        abort(&"Hello");
    }

    pub fn bytestring_failure(&self) -> u32 {
        _bytestring_get_len(&StorageKey::from([72; 32]))
    }

    pub fn do_contract_abort(&self) {
        let buf = [0, 1, 2];

        contract_abort(&buf);
    }
}
