use sdk::*;
use swap::*;

#[contract_exception]
pub enum SwapLibraryError {
    InsufficientAmount = 1,
    InsufficientLiquidity = 2,
}

use SwapLibraryError::*;

// adapted from UniswapV2Library
/// Given an amount of token X, give a price quote for token Y
pub fn quote(amount_x: Amount, reserves: Amounts) -> Amount {
    const X: usize = 0;
    const Y: usize = 1;

    ensure(amount_x > 0, InsufficientAmount);
    ensure(reserves[0] > 0 && reserves[1] > 0, InsufficientLiquidity);

    let amount_y = (U256::from(amount_x) * U256::from(reserves[Y])) / U256::from(reserves[X]);

    amount_y.try_into().unwrap()
}
