/// A port of UniswapV2 from Solidity to groundhog
///
/// Refer to the UniswapV2 [whitepaper] and [v2-core] repository for details.
///
/// [whitepaper](https://docs.uniswap.org/whitepaper.pdf)
/// [v2-core](https://github.com/Uniswap/v2-core)
use crate::erc20::ERC20Client;
use crate::interfaces::ierc20::IERC20;
use crate::util::*;
use core::ops::Mul;
use core::cmp::min;
use sdk::prelude::*;

type Address = ContractAddress;

// This type is a 256-bit unsigned int in UniswapV2, but in this contract,
// values over `U112_MAX` result in runtime failure.  We use `u128` here
// because it is a convenient Rust primitive.
// (For other integer types, we use the same number of bits as in UniswapV2.)
type Amount = u128;

#[derive(Copy, Clone)]
#[non_exhaustive]
pub enum Error {
    InsufficientOutputAmount,
    InsufficientInputAmount,
    InsufficientLiquidity,
    InsufficientLiquidityMinted,
    InvalidDestination,
    InvalidSwapProduct,
    Overflow,
    TransferFailed,
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Default)]
struct ERC20 {
    /* TODO */
}

impl ERC20 {
    pub fn total_supply(&self) -> Amount { todo!() }
    pub fn mint(&self, _to: Address, _amount: Amount) { todo!() }
}

// #[contract]
// #[derive(Serialize, Deserialize)]
#[derive(Default)]
pub struct SwapV2Pair {
    pub factory: Address,
    pub token0: Address,
    pub token1: Address,

    // These private fields are available via `get_reserves()`
    reserve0: U112,
    reserve1: U112,
    block_timestamp_last: u32,

    pub price0_cumulative_last: U256,
    pub price1_cumulative_last: U256,

    /// The product `reserve0` * `reserve1`, as of immediately
    /// after the most recent liquidity event
    pub k_last: U256,

    // An embedded ERC20 contract, implementing the LP token
    // TODO: how to "forward" ERC20 method calls to this?
    lp: ERC20,
}

impl SwapV2Pair {
    pub const MINIMUM_LIQUIDITY: u128 = 10u128.pow(3);
    pub const BLACK_HOLE: Address = Address::zero();

    pub fn new(factory: Address, token0: Address, token1: Address) -> Self {
        Self {
            factory,
            token0,
            token1,

            reserve0: U112::new_strict(0),
            reserve1: U112::new_strict(0),
            block_timestamp_last: 0,

            price0_cumulative_last: 0.into(),
            price1_cumulative_last: 0.into(),
            k_last: 0.into(),

            lp: ERC20::default(),
        }
    }

    pub fn get_reserves(&self) -> (U112, U112, u32) {
        (self.reserve0, self.reserve1, self.block_timestamp_last)
    }

    // Called by LP after transferring tokens to this contract
    pub fn mint(&mut self, to: Address) -> Result<u128> {
        // Current reserve balances
        let balance0 = ERC20Client(&self.token0).balance_of(&this_contract());
        let balance1 = ERC20Client(&self.token1).balance_of(&this_contract());

        // Differences from previously-recorded reserve balances
        let amount0 = balance0.strict_sub(self.reserve0.into());
        let amount1 = balance1.strict_sub(self.reserve1.into());

        let fee_on = self._mint_fee(); // calls _mint when true

        let total_supply = self.lp.total_supply();

        // Added liquidity (TODO: check integer size needs)
        let liquidity = if total_supply == 0 {

            // Permanently lock the first MINIMUM_LIQUIDITY tokens
            self.lp.mint(Self::BLACK_HOLE, Self::MINIMUM_LIQUIDITY);

            amount0.strict_mul(amount1)
                   .isqrt()
                   .strict_sub(Self::MINIMUM_LIQUIDITY)
        } else {
            min(amount0.strict_mul(total_supply)
                       .strict_div(self.reserve0.into()),
                amount1.strict_mul(total_supply)
                       .strict_div(self.reserve1.into()))
        };

        require(liquidity > 0, Error::InsufficientLiquidityMinted)?;

        // Mint the new liquidity to the LP
        self.lp.mint(to, liquidity);

        // Record new balances
        self._update(balance0, balance1)?;

        if fee_on {
            // TODO: check mul?
            self.k_last = U256::from(self.reserve0).mul(U256::from(self.reserve1));
        }

        log(b"MINT").unwrap(); // emit Mint(msg.sender, amount0, amount1)

        Ok(liquidity)
    }

    fn _mint_fee(&mut self) -> bool {
        // Optional TODO: support fee_to setting

        if !self.k_last.is_zero() {
            self.k_last = 0.into();
        }

        false
    }

    pub fn swap(
        &mut self,
        amount0_out: Amount,
        amount1_out: Amount,
        to: &Address,
    ) -> Result<()> {
        require(
            amount0_out > 0 || amount1_out > 0,
            Error::InsufficientOutputAmount,
        )?;

        require(
            amount0_out < self.reserve0.into()
                && amount1_out < self.reserve1.into(),
            Error::InsufficientLiquidity,
        )?;

        require(
            to != &self.token0 && to != &self.token1,
            Error::InvalidDestination,
        )?;

        if amount0_out > 0 {
            self._transfer(0, to, amount0_out)?;
        }
        if amount1_out > 0 {
            self._transfer(1, to, amount1_out)?;
        }

        // Optional TODO: Callback for flash-swap goes here.
        //
        // If it is implemented, we will need something like `#[contract(no_reenter)]`
        // on this method.

        let this_contract = this_contract();
        let balance0: Amount =
            ERC20Client(&mut self.token0).balance_of(&this_contract);
        let balance1: Amount =
            ERC20Client(&mut self.token1).balance_of(&this_contract);

        // The unchecked subtractions here will not wrap, due to a `require` above
        let amount0_in =
            balance0.saturating_sub(Amount::from(self.reserve0) - amount0_out);
        let amount1_in =
            balance1.saturating_sub(Amount::from(self.reserve1) - amount1_out);

        require(
            amount0_in > 0 || amount1_in > 0,
            Error::InsufficientInputAmount,
        )?;

        let balance0_adj = balance0.strict_mul(1000).strict_sub(amount0_in * 3);
        let balance1_adj = balance1.strict_mul(1000).strict_sub(amount1_in * 3);
        let balance_prod = U256::from(balance0_adj).mul(U256::from(balance1_adj));

        let reserve0_adj = u128::from(self.reserve0) * 1000;
        let reserve1_adj = u128::from(self.reserve1) * 1000;
        let reserve_prod = U256::from(reserve0_adj).mul(U256::from(reserve1_adj));

        require(balance_prod >= reserve_prod, Error::InvalidSwapProduct)?;

        self._update(balance0, balance1)?;

        // emit Swap(msg.sender, amount0In, amount1In, amount0Out, amount1Out, to);
        log(b"SWAP").unwrap();

        Ok(())
    }

    fn _transfer(
        &mut self,
        which: u8,
        to: &Address,
        amount: Amount,
    ) -> Result<()> {
        let token = if which == 0 {
            &mut self.token0
        } else {
            &mut self.token1
        };

        let success = ERC20Client(token).transfer(to, amount);

        require(success, Error::TransferFailed)
    }

    fn _update(&mut self, balance0: Amount, balance1: Amount) -> Result<()> {
        // If this fails, `skim()` may be used to extract the overflow
        require(
            balance0 <= U112_MAX && balance1 <= U112_MAX,
            Error::Overflow,
        )?;

        // We intentionally truncate here, desiring the timestamp mod 2^32
        let block_timestamp = get_block_timestamp() as u32;

        // Wrapping yields the right elapsed time if this method is
        // run at least once every ~139 years.
        let time_elapsed =
            block_timestamp.wrapping_sub(self.block_timestamp_last);

        if time_elapsed > 0 && self.reserve0 != 0 && self.reserve1 != 0 {
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

        log(b"SYNC").unwrap(); // emit Sync(reserve0, reserve1);

        Ok(())
    }
}

fn this_contract() -> Address {
    get_current_address().unwrap()
}

fn get_block_timestamp() -> u64 {
    unimplemented!()
}

fn require(cond: bool, err: Error) -> Result<()> {
    if cond {
        Ok(())
    } else {
        Err(err)
    }
}

// Compute exactly: (a * Q) / (b * Q) * p, where Q = 2^112
pub fn uq112x112_div_mul(a: U112, b: U112, p: u32) -> U256 {
    const LN_Q: U256 = U256([112, 0, 0, 0]);

    let a: U256 = a.into();
    let b: U256 = b.into();
    let p: U256 = (p as u128).into();
    (a << LN_Q) / (b << LN_Q) * p
}
