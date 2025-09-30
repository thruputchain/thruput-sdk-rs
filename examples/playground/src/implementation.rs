//! Storage tests

use sdk::*;
pub use sdk::contract_result::*;
use sdk::storage::{Bytestring, NNInt, Handle};

#[storage]
pub struct MyContract {
    contract_owner: ContractAddress,
    silly: bool,
    frottage: Handle<Frottage>,
    hbool1: Handle<bool>,
}

#[storage]
pub struct Frottage {
    touched: bool,
    count: NNInt,
    bytes: Bytestring,
}

type Amount = i128;

#[contract(exports = "guest_exports", err_type = Error, client = MyClient)]
impl MyContract {
    // TODO: attribute #[init]: panic if storage already present; otherwise provide default value
    pub fn initialize(&mut self, contract_owner: &ContractAddress) -> Result<()> {
        self.contract_owner = contract_owner.clone();

        Ok(())
    }

    pub fn hello(&mut self, buf: &[u8]) -> Result<u32> {
        Ok(buf.len() as u32)
    }

    pub fn add_frot(&mut self, amount: Amount) -> Result<()> {
        let mut frottage = self.frottage.load_mut()?;

        assert!(frottage.touched == false, "Shouldn't be touched");

        frottage.touched = true;
        frottage.bytes.set(&[0, 1, 2, 3, 4]);
        frottage.count.add_default_zero(amount as i128)?;

        let mut hbool1 = self.hbool1.load_mut()?;
        assert_eq!(*hbool1, false);
        *hbool1 = true;

        Ok(())
        // Drop causes flush
    }

    pub fn get_frot(&self) -> Result<Amount> {
        let hbool1 = self.hbool1.load()?;
        assert_eq!(*hbool1, true);

        let frottage = self.frottage.load()?;

        assert!(frottage.touched == true, "Should be touched");

        let bytes = frottage.bytes.get();
        assert!(&*bytes == &[0, 1, 2, 3, 4], "Bad bytes");

        let count = frottage.count.get()?;
        Ok(count as Amount)
    }
}

#[allow(dead_code)]
#[contract(exports = "guest_exports")]
pub fn free_function(n: u32, _addr: &ContractAddress) -> Result<u32> {
    Ok(n)
}
