use sdk::storage::NNInt;
use sdk::*;

#[contract_exception]
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
    pub fn initialize(&mut self, initial_value: u64) {
        self.value.strict_inc(initial_value as i128);
    }

    pub fn increment(&mut self, amount: u64) {
        self.value.strict_inc(amount as i128);
    }

    pub fn decrement(&mut self, amount: u64) {
        self.value.strict_inc(-(amount as i128));
    }

    pub fn get(&self) -> i128 {
        self.value.try_get().none_if_missing().unwrap_or(0)
    }
}
