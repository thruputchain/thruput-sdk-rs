//! A contract implementing ISwapV2Pair and ERC20.

use crate::bignum::{U112, U256};
use crate::interfaces::iswapv2_pair::*;
use core::cmp::min;
use core::ops::Mul;
use erc20::TokenContract; // The ERC20 implementation
use groundhog_erc20_if::{ERC20, ERC20Error, Amount, Difference};
use erc20::ERC20Client;
use sdk::*;

// We need to derive `Storage` because TokenContract is `Storage`
#[storage]
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
    lp: TokenContract,
}

// Helper methods
impl SwapV2Pair {
    pub const MINIMUM_LIQUIDITY: u128 = 10u128.pow(3);
    pub const BLACK_HOLE: Address = Address::zero();

    fn _mint_fee(&mut self) -> bool {
        // Optional TODO: support fee_to setting

        if !self.k_last.is_zero() {
            self.k_last = U256::zero();
        }

        false
    }

    fn _get_token_balances(&self) -> (Amount, Amount) {
        wrap_erc20(|| {
            let balance0 = ERC20Client::new(&self.token0)
                .get_balance_of(this_contract());

            let balance1 = ERC20Client::new(&self.token1)
                .get_balance_of(this_contract());

            (balance0, balance1)
        })
    }

    fn _transfer(
        &mut self,
        which: u8,
        to: &Address,
        amount: Amount,
    ) {
        let token = if which == 0 {
            &mut self.token0
        } else {
            &mut self.token1
        };

        let mut token = ERC20Client::new(token);

        wrap_erc20_with(|_| TransferFailed, || token.transfer(to, amount));
    }

    fn _update(&mut self, balance0: Amount, balance1: Amount) {
        // If this fails, `skim()` may be used to extract the overflow
        self.require(
            balance0 <= U112::MAX && balance1 <= U112::MAX,
            Overflow,
        );

        // We intentionally truncate here, desiring the timestamp mod 2^32
        let block_timestamp = get_block_timestamp() as u32;

        // Wrapping yields the right elapsed time if this method is
        // run at least once every ~139 years.
        let time_elapsed =
            block_timestamp.wrapping_sub(self.block_timestamp_last);

        if time_elapsed > 0 && self.reserve0 != 0 && self.reserve1 != 0 {
            self.price0_cumulative_last +=
                uq112x112_div_mul(self.reserve1, self.reserve0, time_elapsed);

            self.price1_cumulative_last +=
                uq112x112_div_mul(self.reserve0, self.reserve1, time_elapsed);
        }

        // These will not fail, due to the checks at the top
        // of this function
        self.reserve0 = U112::try_from(balance0).unwrap();
        self.reserve1 = U112::try_from(balance1).unwrap();

        self.block_timestamp_last = block_timestamp;

        log(b"SYNC(reserve0, reserve1)"); // XXX: emit event
    }

    // We are passing self in the remaining methods below because, with some
    // SDK changes, we might want to use the contract's Throws trait to abort:

    fn abort(&self, e: &ISwapV2PairError) -> ! {
        sdk::abort(e)
    }

    fn require(&self, check: bool, e: ISwapV2PairError) {
        if !check {
            self.abort(&e)
        }
    }
}

// Run the provided function.  If it aborts with an ERC20Error,
// wrap it before aborting.
//
fn wrap_erc20_with<T, F, W>(wrap: W, f: F) -> T
where
    W: Fn(ERC20Error) -> ISwapV2PairError + 'static,
    F: FnOnce() -> T,
{
    catch(move |e: &ERC20Error| abort(&wrap(e.clone())), f)
}


fn wrap_erc20<T, F>(f: F) -> T
where
    F: FnOnce() -> T,
{
    wrap_erc20_with(LPTokenFailure, f)
}

impl AsRef<TokenContract> for SwapV2Pair {
    fn as_ref(&self) -> &TokenContract {
        &self.lp
    }
}

impl AsMut<TokenContract> for SwapV2Pair {
    fn as_mut(&mut self) -> &mut TokenContract {
        &mut self.lp
    }
}

// TODO: use delegate?
#[contract(throws = ERC20Error, version = "0.2.0", exports = "erc20_exports", test_setup = "erc20_test_setup")]
impl ERC20 for SwapV2Pair {
    fn initialize(
        &mut self,
        _contract_owner: &ContractAddress,
    ) {
        // Embedded contract does not need initialization
        abort(&ERC20Error::AlreadyInitialized);
    }

    fn transfer(
        &mut self,
        receiver: &ContractAddress,
        amount: Amount,
    ) {
        self.lp.transfer(receiver, amount);
    }

    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
    ) {
        self.lp.transfer_from(owner, receiver, amount);
    }

    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        diff: Difference,
    ) {
        self.lp.adjust_allowance(spender, diff);
    }

    fn allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Amount {
        self.lp.allowance(owner, spender)
    }

    fn get_balance_of(&self, addr: &ContractAddress) -> Amount {
        self.lp.get_balance_of(addr)
    }

    fn get_balance(&self) -> Amount {
        self.lp.get_balance()
    }

    fn mint(
        &mut self,
        receiver: &ContractAddress,
        amount: Amount,
    ) {
        self.lp.mint(receiver, amount);
    }

    fn burn(
        &mut self,
        owner: &ContractAddress,
        amount: Amount,
    ) {
        self.lp.mint(owner, amount);
    }

    fn total_supply(&self) -> Amount {
        self.lp.total_supply()
    }
}

#[contract(throws = ISwapV2PairError, exports = "swap_exports", test_setup = "swap_test_setup")]
impl ISwapV2Pair for SwapV2Pair {
    fn initialize(
        &mut self,
        factory: &Address,
        token0: &Address,
        token1: &Address,
    ) {
        self.factory = factory.clone();
        self.token0 = token0.clone();
        self.token1 = token1.clone();

        let lp_contract_owner = factory; // or this_contract() ?
        self.lp.initialize(lp_contract_owner);
    }

    fn get_reserves(&self) -> (U112, U112, u32) {
        (self.reserve0, self.reserve1, self.block_timestamp_last)
    }

    fn get_liquidity(&self) -> Amount {
        wrap_erc20(|| self.lp.total_supply())
    }

    // Mint LP in exchange for any pair-tokens transferred
    fn mint_lp(&mut self, to: &Address) -> u128 {
        // New reserve balances after LP transfer
        let (balance0, balance1) = self._get_token_balances();

        // Differences from previously-recorded reserve balances
        let amount0 = balance0.strict_sub(self.reserve0.into());
        let amount1 = balance1.strict_sub(self.reserve1.into());

        // NOTE: This check not present in UniswapV2
        self.require(
            amount0 > 0 && amount1 > 0,
            ISwapV2PairError::InsufficientLiquidityMinted,
        );

        let fee_on = self._mint_fee();

        let total_liquidity = wrap_erc20(|| self.lp.total_supply());

        // Added liquidity (TODO: check integer size needs)
        let liquidity = if total_liquidity == 0 {
            // Permanently lock the first MINIMUM_LIQUIDITY tokens
            wrap_erc20(|| self.lp.mint(&Self::BLACK_HOLE, Self::MINIMUM_LIQUIDITY));

            amount0
                .strict_mul(amount1)
                .isqrt()
                .strict_sub(Self::MINIMUM_LIQUIDITY)
        } else {
            min(
                amount0
                    .strict_mul(total_liquidity)
                    .strict_div(self.reserve0.into()),
                amount1
                    .strict_mul(total_liquidity)
                    .strict_div(self.reserve1.into()),
            )
        };

        self.require(liquidity > 0, InsufficientLiquidityMinted);

        // Mint the new liquidity to the address specified by the LP
        wrap_erc20(|| self.lp.mint(to, liquidity));

        // Record the new reserve balances
        self._update(balance0, balance1);

        if fee_on {
            // TODO: check mul?
            self.k_last =
                U256::from(self.reserve0).mul(U256::from(self.reserve1));
        }

        log(b"MINT"); // XXX: instead, emit event Mint(msg.sender, amount0, amount1)

        // Return the amount of liquidity minted
        liquidity
    }

    // Transfer pair-tokens in exchange for any LP tokens returned to us
    fn burn_lp(&mut self, to: &Address) -> (Amount, Amount) {
        let (balance0, balance1) = self._get_token_balances();

        let liquidity = wrap_erc20(|| self.lp.get_balance_of(this_contract()));

        let fee_on = self._mint_fee();
        let total_liquidity = wrap_erc20(|| self.lp.total_supply());

        // NOTE: added this check
        self.require(total_liquidity > 0, NoLiquidityToBurn);

        let amount0 = liquidity.strict_mul(balance0) / total_liquidity;
        let amount1 = liquidity.strict_mul(balance1) / total_liquidity;
        self.require(
            amount0 > 0 && amount1 > 0,
            InsufficientLiquidityBurned,
        );
        wrap_erc20(|| self.lp.burn(this_contract(), liquidity));
        self._transfer(0, &to, amount0);
        self._transfer(1, &to, amount1);
        let (balance0, balance1) = self._get_token_balances();

        self._update(balance0, balance1);
        if fee_on {
            self.k_last =
                U256::from(self.reserve0).mul(U256::from(self.reserve1));
        }
        log(b"BURN"); // XXX: instead, emit event Burn(msg.sender, amount0, amount1, to);

        // Return amounts burned
        (amount0, amount1)
    }

    fn swap(
        &mut self,
        amount0_out: Amount,
        amount1_out: Amount,
        to: &Address,
    ) {
        self.require(
            amount0_out > 0 || amount1_out > 0,
            InsufficientOutputAmount,
        );

        self.require(
            amount0_out < self.reserve0.into()
                && amount1_out < self.reserve1.into(),
            InsufficientLiquidity,
        );

        self.require(
            to != &self.token0 && to != &self.token1,
            InvalidDestination,
        );

        if amount0_out > 0 {
            self._transfer(0, to, amount0_out);
        }
        if amount1_out > 0 {
            self._transfer(1, to, amount1_out);
        }

        // Optional TODO: Callback for flash-swap goes here.
        //
        // If it is implemented, we will need something like `#[contract(no_reenter)]`
        // on this method.

        let (balance0, balance1) = self._get_token_balances();

        // The unchecked subtractions here will not wrap, due to a `require` above
        let amount0_in =
            balance0.saturating_sub(Amount::from(self.reserve0) - amount0_out);
        let amount1_in =
            balance1.saturating_sub(Amount::from(self.reserve1) - amount1_out);

        self.require(
            amount0_in > 0 || amount1_in > 0,
            InsufficientInputAmount,
        );

        let balance0_adj =
            balance0.strict_mul(1000).strict_sub(amount0_in * 3);
        let balance1_adj =
            balance1.strict_mul(1000).strict_sub(amount1_in * 3);
        let balance_prod =
            U256::from(balance0_adj).mul(U256::from(balance1_adj));

        let reserve0_adj = u128::from(self.reserve0) * 1000;
        let reserve1_adj = u128::from(self.reserve1) * 1000;
        let reserve_prod =
            U256::from(reserve0_adj).mul(U256::from(reserve1_adj));

        self.require(balance_prod >= reserve_prod, InvalidSwapProduct);

        self._update(balance0, balance1);

        // XXX: instead, emit Swap(msg.sender, amount0In, amount1In, amount0Out, amount1Out, to);
        log(b"SWAP");
    }

    // Extract any reserve balances surplus to what was last recorded
    fn skim(&mut self, to: &Address) {
        let (balance0, balance1) = self._get_token_balances();

        let surplus0 = balance0.strict_sub(self.reserve0.into());
        let surplus1 = balance1.strict_sub(self.reserve1.into());

        self._transfer(0, &to, surplus0);
        self._transfer(1, &to, surplus1);
    }

    // Record current reserve balances
    fn sync(&mut self) {
        let (balance0, balance1) = self._get_token_balances();

        self._update(balance0, balance1);
    }
}

// Compute exactly: (a * Q) / (b * Q) * p, where Q = 2^112
pub fn uq112x112_div_mul(a: U112, b: U112, p: u32) -> U256 {
    const LN_Q: U256 = [112u64, 0, 0, 0].into(); // XXX this can't be right

    let a: U256 = a.into();
    let b: U256 = b.into();
    let p: U256 = p.into();
    (a << LN_Q) / (b << LN_Q) * p
}

fn this_contract() -> &'static ContractAddress {
    static mut ADDRESS_CELL: Option<ContractAddress> = None;

    #[allow(static_mut_refs)]
    // SAFETY: ??
    unsafe {
        if ADDRESS_CELL.is_none() {
            let address = get_current_address();
            ADDRESS_CELL = Some(address);
        }
        ADDRESS_CELL.as_ref().unwrap()
    }
}

fn get_block_timestamp() -> u64 {
    42
}
