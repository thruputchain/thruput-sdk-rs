pub const DECIMALS_LOG2: u32 = 30; // ~= 9 decimals
pub const DECIMAL_FACTOR: u128 = 2u128.pow(DECIMALS_LOG2);

// Give a floating-point approximation of a token amount divided by a
// well-known factor.
//
// TODO: Add decimal factor constant to ERC20 contract?
//
pub fn decimalized(n: u128) -> f64 {
    let whole: u32 = n.strict_shr(DECIMALS_LOG2).try_into().unwrap();
    let whole: f64 = whole.into();

    let rem: u32 = (n % DECIMAL_FACTOR).try_into().unwrap();
    let rem: f64 = rem as f64 / DECIMAL_FACTOR as f64;

    whole + rem
}

// Return a token amount approximately corresponding to a floating-point amount
// times the decimal factor.  (For test rigs.)
//
pub const fn undecimalize(f: f64) -> u128 {
    (f * DECIMAL_FACTOR as f64) as u128
}
