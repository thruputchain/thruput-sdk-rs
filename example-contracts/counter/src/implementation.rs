use sdk::storage::NNInt;
use sdk::*;

#[contract_error]
pub enum Error {
    NotAuthorized = 1,
}

#[derive(Debug)]
#[sdk::storage]
pub struct Counter {
    value: NNInt,
}

#[contract(throws = Error, client = CounterClient)]
impl Counter {
    #[init]
    pub fn initialize(&mut self, initial_value: u64) {
        self.value.strict_inc(initial_value as i128);
    }

    pub fn increment(&mut self, amount: u64) {
        self.value.strict_inc(amount as i128);
    }

    pub fn decrement(&mut self, amount: u64) {
        self.value.strict_inc(-(amount as i128));
    }

    pub fn get(&self) -> u64 {
        self.value.try_get().unwrap_or(0) as u64
    }
}
