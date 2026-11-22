use thruput_sdk::storage::Value;
use thruput_sdk::*;

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

    /// `floor(sqrt(value))`. In the rv32 artifact the root comes from the
    /// runtime, through the `U256Sqrt` host operation.
    pub fn sqrt(&self, value: U256) -> U256 {
        value.integer_sqrt()
    }

    /// `ceil(sqrt(value))`: the rounding variant, over the same host
    /// operation.
    pub fn sqrt_ceil(&self, value: U256) -> U256 {
        value.sqrt_rounding(Rounding::Ceil)
    }

    /// `x * y / d` rounded down, or `None` when `d` is zero or the quotient
    /// does not fit. In the rv32 artifact the division is the runtime's,
    /// through the `U256MulDivRem` host operation.
    pub fn mul_div(&self, x: U256, y: U256, d: U256) -> Option<U256> {
        U256::checked_mul_div(x, y, d)
    }

    /// `x * y / d` rounded up: the rounding variant, over the same host
    /// operation.
    pub fn mul_div_ceil(&self, x: U256, y: U256, d: U256) -> Option<U256> {
        U256::checked_mul_div_rounding(x, y, d, Rounding::Ceil)
    }

    /// `(x * y) % m`, or `None` when `m` is zero.
    pub fn mul_mod(&self, x: U256, y: U256, m: U256) -> Option<U256> {
        U256::checked_mul_mod(x, y, m)
    }

    /// `b^e % m`, or `None` when `m` is zero: a loop over `mul_mod`.
    pub fn mod_exp(&self, b: U256, e: U256, m: U256) -> Option<U256> {
        U256::checked_mod_exp(b, e, m)
    }

    /// `a / d`, by the operator. In the rv32 artifact that is the runtime's
    /// `U256MulDivRem` with a multiplier of one.
    pub fn div(&self, a: U256, d: U256) -> U256 {
        a / d
    }

    /// `a % d`, by the operator, over the same host operation.
    pub fn rem(&self, a: U256, d: U256) -> U256 {
        a % d
    }

    /// The signed `x * y / d`, truncated, or `None` when `d` is zero or the
    /// quotient is out of range. Its magnitudes go through `U256MulDivRem`.
    pub fn signed_mul_div(&self, x: I256, y: I256, d: I256) -> Option<I256> {
        I256::checked_mul_div(x, y, d)
    }

    /// `a * b / d` over `u128`, or `None` when `d` is zero or the quotient
    /// does not fit. In the rv32 artifact it is the runtime's
    /// `U128MulDivRem`.
    pub fn mul_div_u128(&self, a: u128, b: u128, d: u128) -> Option<u128> {
        checked_mul_div_u128(a, b, d)
    }
}
