use core::fmt;
pub use groundhog_erc20_if::ERC20Error;
use sdk::*;

pub use crate::bignum::{U112, U256};

#[contract_exception]
#[non_exhaustive]
pub enum ISwapV2PairError {
    InitializeFailed = 1,
    AlreadyInitialized = 2,
    NoLiquidityToBurn = 3,
    InsufficientOutputAmount = 4,
    InsufficientInputAmount = 5,
    InsufficientLiquidity = 6,
    InsufficientLiquidityMinted = 7,
    InsufficientLiquidityBurned = 8,
    InvalidDestination = 9,
    InvalidSwapProduct = 10,
    Overflow = 11,
    TransferFailed = 12,
    LPTokenFailure(ERC20Error) = 13,
}

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

// A pair of amounts, corresponding to the two tokens in a swap pair.
pub type Amounts = [Amount; 2];

pub type Address = sdk::ContractAddress;
pub type Addresses = [sdk::ContractAddress; 2];

#[contract(client = ISwapV2PairClient, throws(ISwapV2PairError, ERC20Error))]
pub trait ISwapV2Pair {
    // event Approval
    // event Transfer

    /// Must be called exactly once before any other methods
    fn initialize(
        &mut self,
        factory: &Address,
        token0: &Address,
        token1: &Address,

        // The hash of an ERC20 token contract.  This method will deploy the
        // contract to manage the LP token.
        lp_token_hash: &SHA256Hash,
    );

    fn liquidity_token(&self) -> ContractAddress;

    fn get_reserves(&self) -> (u128, u128, u32);
    fn get_liquidity(&self) -> Amount;

    fn mint_lp(&mut self, to: &Address) -> Amount;
    fn burn_lp(&mut self, to: &Address) -> Amounts;

    fn skim(&mut self, to: &Address);
    fn sync(&mut self);

    fn swap(&mut self, amount0_out: Amount, amount1_out: Amount, to: &Address);

    // Driver helpers

    fn handle_purchase(&mut self, purchaser: &ContractAddress, amounts_in: Amounts, recipient: &ContractAddress);
    fn handle_redeem(&mut self, redeemer: &ContractAddress);
    fn handle_swap(&mut self, trader: &ContractAddress, amounts_out: Amounts);
}
