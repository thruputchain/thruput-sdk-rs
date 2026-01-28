use sdk::*;

#[contract(client = MyClient)]
pub mod my_contract {
    pub fn initialize(contract_owner: &ContractAddress) {
        let key = StorageKey::default();

        // Store ourselves
        bytestring_set(&key, contract_owner.as_ref()).unwrap();
    }
}
