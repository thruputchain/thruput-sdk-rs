use core::fmt;
use sdk::contract;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use crate::bignum::{U112, U256};

#[derive(Error, Debug, PartialEq, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Error {
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
    PairTokenFail, // temporary
    LPTokenFailure(erc20::interface::Error),
}

// TODO: Wrap the above error type in one that can also
// represent sdk errors

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SwapV2{:?}", self)
    }
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

// This type is a 256-bit unsigned int in UniswapV2, but in this contract,
// values over `U112_MAX` result in runtime failure.  We use `u128` here
// because it is a convenient Rust primitive.
// (For other integer types, we use the same number of bits as in UniswapV2.)
pub type Amount = u128;

pub type Address = sdk::prelude::ContractAddress;

#[contract(client = ISwapV2PairClient)]
pub trait ISwapV2Pair {
    // event Approval
    // event Transfer

    // Must be called exactly once before any other methods
    fn initialize(
        &mut self,
        factory: &Address,
        token0: &Address,
        token1: &Address,
    ) -> Result<()>;

    fn get_reserves(&self) -> Result<(U112, U112, u32)>;
    // TODO: macro-generate getters for public members?

    fn get_liquidity(&self) -> Result<Amount>;

    // These names need to not collide with ERC20::{mint, burn} because
    // the swap contract will implement that interface also
    fn mint_lp(&mut self, to: &Address) -> Result<u128>;
    fn burn_lp(&mut self, to: &Address) -> Result<(Amount, Amount)>;

    fn swap(
        &mut self,
        amount0_out: Amount,
        amount1_out: Amount,
        to: &Address,
    ) -> Result<()>;
    fn skim(&mut self, to: &Address) -> Result<()>;
    fn sync(&mut self) -> Result<()>;
}
