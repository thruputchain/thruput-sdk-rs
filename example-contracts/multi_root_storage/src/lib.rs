#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

pub use sdk::storage::*;
pub use sdk::*;

#[derive(Storage)]
pub struct Ledger {            // domain defaults to "Ledger"
    balance: Value<u64>,
}

#[derive(Storage)]
pub struct Registry {         // domain defaults to "Registry"
    count: Value<u64>,
}

#[contract]
impl Ledger {
    pub fn set_balance(&mut self, v: u64) { self.balance.set(&v); }
    pub fn balance(&self) -> u64 { self.balance.get() }
}

#[contract]
impl Registry {
    pub fn bump(&mut self) {
        let n = self.count.get();
        self.count.set(&(n + 1));
    }
    pub fn count(&self) -> u64 { self.count.get() }
}
