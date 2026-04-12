use core::fmt;
use sdk::*;

pub use crate::bignum::{U112, U256};

#[contract_exception]
#[non_exhaustive]
pub enum ISwapV2PairError {
    InitializeFailed,
    AlreadyInitialized,
    NoLiquidityToBurn,
    InsufficientOutputAmount,
    InsufficientInputAmount,
    InsufficientLiquidity,
    InsufficientLiquidityMinted,
    InsufficientLiquidityBurned,
    InvalidDestination,
    InvalidSwapProduct,
    Overflow,
    TransferFailed,
    LPTokenFailure(groundhog_erc20_if::ERC20Error),
}

pub use ISwapV2PairError::*;

impl fmt::Display for ISwapV2PairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SwapV2{:?}", self)
    }
}

// This type is a 256-bit unsigned int in UniswapV2, but in this contract,
// values over `U112_MAX` result in runtime failure.  We use `u128` here
// because it is a convenient Rust primitive.
// (For other integer types, we use the same number of bits as in UniswapV2.)
pub type Amount = u128;

pub type Address = sdk::ContractAddress;

#[contract(client = ISwapV2PairClient, throws = ISwapV2PairError)]
pub trait ISwapV2Pair {
    // event Approval
    // event Transfer

    // Must be called exactly once before any other methods
    fn initialize(
        &mut self,
        factory: &Address,
        token0: &Address,
        token1: &Address,
    );

    fn get_reserves(&self) -> (U112, U112, u32);
    fn get_liquidity(&self) -> Amount;

    fn mint_lp(&mut self, to: &Address) -> u128;
    fn burn_lp(&mut self, to: &Address) -> (Amount, Amount);

    fn swap(&mut self, amount0_out: Amount, amount1_out: Amount, to: &Address);
    fn skim(&mut self, to: &Address);
    fn sync(&mut self);
}
