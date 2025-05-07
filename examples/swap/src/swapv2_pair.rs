/// A port of UniswapV2 from Solidity to groundhog
///
/// Refer to the UniswapV2 [whitepaper] and [v2-core] repository for details.
///
/// [whitepaper](https://docs.uniswap.org/whitepaper.pdf)
/// [v2-core](https://github.com/Uniswap/v2-core)

#![allow(dead_code)]

use sdk::prelude::*;
use crate::erc20::ERC20;
use crate::util::*;

type Address = ContractAddress;

// This type is a 256-bit unsigned int in UniswapV2, but in this contract,
// values over `U112_MAX` result in runtime failure.  We use `u128` here
// because it is a convenient Rust primitive.
// (For other integer types, we use the same number of bits as in UniswapV2.)
type Amount = u128;

#[derive(Copy, Clone)]
pub enum Error {
    InsufficientOutputAmount,
    InsufficientInputAmount,
    InsufficientLiquidity,
    InvalidDestination,
    InvalidSwapProduct,
    Overflow,
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

// #[contract]
// #[derive(Serialize, Deserialize)]
#[derive(Default)]
pub struct SwapV2Pair {
    pub factory: Address,
    pub token0: Address,
    pub token1: Address,

    reserve0: U112,
    reserve1: U112,
    block_timestamp_last: u32,

    pub price0_cumulative_last: U256,
    pub price1_cumulative_last: U256,

    /// The product `reserve0` * `reserve1`, as of immediately
    /// after the most recent liquidity event
    pub k_last: U256,
}

// We want to impl these:
//  crate::interfaces::ISwapV2Pair
//  crate::interfaces::ERC20

impl SwapV2Pair {

    pub const MINIMUM_LIQUIDITY: u32 = 10u32.pow(3);

    pub fn new(factory: Address, token0: Address, token1: Address) -> Self {
        Self {
            factory,
            token0,
            token1,

            reserve0: 0,
            reserve1: 0,
            block_timestamp_last: 0,

            price0_cumulative_last: 0,
            price1_cumulative_last: 0,
            k_last: 0,
        }
    }

    pub fn get_reserves(&self) -> (U112, U112, u32) {
        (self.reserve0, self.reserve1, self.block_timestamp_last)
    }

    // TODO: #[contract(no_reenter)]
    pub fn swap(
        &mut self,
        amount0_out: Amount,
        amount1_out: Amount,
        to: Address)
    -> Result<()> {
        require(amount0_out > 0 || amount1_out > 0,
            Error::InsufficientOutputAmount)?;

        require(amount0_out < self.reserve0 && amount1_out < self.reserve1,
            Error::InsufficientLiquidity)?;

        require(to != self.token0 && to != self.token1,
            Error::InvalidDestination)?;

        if amount0_out > 0 {
            self._transfer(0, to, amount0_out);
        }
        if amount1_out > 0 {
            self._transfer(1, to, amount1_out);
        }

        // Callback for flash-swap goes here

        let this_contract = get_current_address();
        let balance0: Amount = IERC20(&self.token0).balance_of(this_contract);
        let balance1: Amount = IERC20(&self.token1).balance_of(this_contract);

        let amount0_in = balance0.saturating_sub(self.reserve0 - amount0_out);
        let amount1_in = balance1.saturating_sub(self.reserve1 - amount1_out);

        require(amount0_in > 0 || amount1_in > 0,
            Error::InsufficientInputAmount)?;

        let balance0_adj = balance0.strict_mul(1000).strict_sub(amount0_in * 3);
        let balance1_adj = balance1.strict_mul(1000).strict_sub(amount1_in * 3);
        let balance_prod: U256 = balance0_adj.into().mul(balance1_adj);

        let reserve0_adj: u128 = self.reserve0.into() * 1000;
        let reserve1_adj: u128 = self.reserve1.into() * 1000;
        let reserve_prod: U256 = reserve0_adj.into().mul(reserve1_adj);

        require(balance_prod >= reserve_prod, Error::InvalidSwapProduct)?;

        self._update(balance0, balance1);

        // emit Swap(msg.sender, amount0In, amount1In, amount0Out, amount1Out, to);

        Ok(())
    }

    fn _transfer(&self, which: u8, to: Address, amount: Amount) {
        let token = if (which == 0) { &self.token0 } else { &self.token1 };

        IERC20(token).transfer(to, amount);
    }

    fn _update(
        &mut self,
        balance0: Amount,
        balance1: Amount,
    ) {
        // If this fails, `skim()` may be used to extract the overflow
        require(balance0 <= U112_MAX && balance1 <= U112_MAX,
            Error::Overflow);

        // We intentionally truncate here, desiring the timestamp mod 2^32
        let block_timestamp = get_block_timestamp() as u32;

        // Wrapping yields the right elapsed time if this method is
        // run at least once every ~139 years.
        let time_elapsed = block_timestamp.wrapping_sub(self.block_timestamp_last);

        if (time_elapsed > 0 && self.reserve0 != 0 && self.reserve1 != 0) {

            // Overflow is desired

            self.price0_cumulative_last += uq112x112_div_mul(
                self.reserve1,
                self.reserve0,
                time_elapsed,
            );

            self.price1_cumulative_last += uq112x112_div_mul(
                self.reserve0,
                self.reserve1,
                time_elapsed,
            );
        }

        // These will not fail, due to the checks at the top
        // of this function
        self.reserve0 = U112::try_from(balance0).unwrap();
        self.reserve1 = U112::try_from(balance1).unwrap();

        self.block_timestamp_last = block_timestamp;

        // emit Sync(reserve0, reserve1);
    }
}

fn get_block_timestamp() -> u64 {
    todo!()
}

fn require(cond: bool, err: Error) -> Result<()> {
    if cond {
        Ok(())
    } else {
        Err(err)
    }
}
