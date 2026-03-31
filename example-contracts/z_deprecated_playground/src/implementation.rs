//! Storage tests

pub use sdk::contract_result::*;
use sdk::storage::{Bytestring, NNInt, Value};
use sdk::*;

#[storage]
pub struct MyContract {
    contract_owner: Value<ContractAddress>,
    silly: Value<bool>,
    frottage: Frottage,
    hbool1: Value<bool>,
}

#[storage]
pub struct Frottage {
    touched: Value<bool>,
    count: NNInt,
    bytes: Bytestring,
}

type Amount = i128;

#[contract(exports = "guest_exports", err_type = Error, client = MyClient)]
impl MyContract {
    #[init]
    pub fn initialize(
        &mut self,
        contract_owner: &ContractAddress,
    ) -> Result<()> {
        self.contract_owner.set(contract_owner);

        Ok(())
    }

    pub fn hello(&mut self, buf: &[u8]) -> Result<u32> {
        Ok(buf.len() as u32)
    }

    pub fn add_frot(&mut self, amount: Amount) -> Result<()> {
        assert!(self.frottage.touched.try_get().unwrap_or(false) == false, "Shouldn't be touched");

        self.frottage.touched.set(&true);
        self.frottage.bytes.set(&[0, 1, 2, 3, 4]);
        self.frottage.count.inc(amount as i128)?;

        assert_eq!(self.hbool1.try_get().unwrap_or(false), false);
        self.hbool1.set(&true);

        Ok(())
    }

    pub fn get_frot(&self) -> Result<Amount> {
        assert_eq!(self.hbool1.get(), true);

        assert!(self.frottage.touched.get() == true, "Should be touched");

        let bytes = self.frottage.bytes.get();
        assert!(&*bytes == &[0, 1, 2, 3, 4], "Bad bytes");

        let count = self.frottage.count.get()?;
        Ok(count as Amount)
    }
}

#[allow(dead_code)]
#[contract(exports = "guest_exports")]
pub fn free_function(n: u32, _addr: &ContractAddress) -> Result<u32> {
    Ok(n)
}
