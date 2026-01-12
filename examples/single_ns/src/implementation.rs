use sdk::*;

// Read/write permanent state

/*
fn get_owner() -> Result<ContractAddress> {
    if let Some(bytes) = bytestring_get_sized(&get_owner_key())? {
        Ok(bytes.into())
    } else {
        internal_err("Unexpected bytestring size")
    }
}
*/

#[derive(Default)]
struct Contract;

#[contract(storage = "false")]
impl Contract {
    pub fn initialize(&mut self, contract_owner: &ContractAddress) {
        let key = StorageKey::default();

        // Store ourselves
        bytestring_set(&key, contract_owner.as_ref()).unwrap();
    }
}
