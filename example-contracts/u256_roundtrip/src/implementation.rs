use sdk::storage::Value;
use sdk::*;

#[storage]
pub struct U256Roundtrip {
    value: Value<U256>,
    signed: Value<I256>,
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

    /// Negate `amount`, persist the result, and return it.
    ///
    /// `wrapping_neg` rather than `-`, because `I256::MIN` has no negation:
    /// the operator would wrap silently in a contract profile anyway, and
    /// saying so is better than looking like it cannot happen.
    pub fn negate_and_store(&mut self, amount: I256) -> I256 {
        let negated = amount.wrapping_neg();
        self.signed.set(&negated);
        negated
    }

    /// The value persisted by the last [`Self::negate_and_store`] call.
    pub fn stored_signed(&self) -> I256 {
        self.signed.get()
    }
}
