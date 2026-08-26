use sdk::storage::Value;
use sdk::*;

#[storage]
pub struct U256Roundtrip {
    value: Value<U256>,
}

#[contract(client = U256RoundtripClient)]
impl U256Roundtrip {
    /// Add one to `amount`, persist the sum, and return it.
    pub fn add_one_and_store(&mut self, amount: U256) -> U256 {
        let sum = amount + U256::ONE;
        self.value.set(&sum);
        sum
    }

    /// The sum persisted by the last [`Self::add_one_and_store`] call.
    pub fn stored(&self) -> U256 {
        self.value.get()
    }
}
