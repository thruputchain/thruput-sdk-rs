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

#[storage]
struct Contract {
    contract_owner: ContractAddress,
}

#[contract]
impl Contract {
    pub fn initialize(&mut self, contract_owner: &ContractAddress) {
        self.contract_owner = contract_owner.clone();
    }
}
