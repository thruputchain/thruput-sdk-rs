//! A contract implementing ISwapV2Pair

use crate::bignum::U256;
use crate::interfaces::iswapv2_pair::*;
use crate::events::*;
use crate::reserves::*;
use core::cmp::min;
use core::iter::zip;
use core::ops::Mul;
use groundhog_erc20_if::ERC20Client;
use sdk::storage::*;
use sdk::*;

/// State for the pair contract
#[storage]
pub struct SwapV2Pair {
    // Configuration: token (A, B) addresses
    tokens: Value<[Address; 2]>,

    // Configuration: A subsidiary ERC20 contract, implementing the LP token.
    lp_addr: Value<Address>,

    // State
    reserves: Value<Reserves>,
}

#[contract(exports = "swap_exports")]
impl ISwapV2Pair for SwapV2Pair {
    fn initialize(
        &mut self,
        _factory: &Address,
        token0: &Address,
        token1: &Address,
        lp_token_hash: &SHA256Hash,
    ) {
        if self.lp_addr.try_get().is_some() {
            throw(ISwapV2PairError::AlreadyInitialized);
        }

        hash_ident! { static LP_NONCE: SHA256Hash; }

        self.lp_addr.set(&deploy_contract(&LP_NONCE, lp_token_hash));
        self.tokens.set(&[*token0, *token1]);
        self.reserves.set(&Reserves::new());

        // Initialize the LP contract, controlled by us (the pair contract)
        self.lp().initialize(&get_current_address());
    }

    fn liquidity_token(&self) -> ContractAddress {
        self.lp_addr.get()
    }

    fn get_reserves(&self) -> (u128, u128, u32) {
        let r = self.reserves.get();
        let [r0, r1] = r.balances();
        (r0.into(), r1.into(), r.block_timestamp())
    }

    fn get_liquidity(&self) -> Amount {
        self.lp().total_supply()
    }

    // Mint LP in exchange for any A/B-tokens transferred
    fn mint_lp(&mut self, to: &Address) -> Amount {
        let mut reserves = self.reserves.get();

        // Get the previous snapshot of our reserve balances
        let prev_balances = reserves.balances();

        // Get the current reserve balances, now that the LP has made transfers to us
        let balances = self._get_token_balances();

        // Record the new balances
        reserves.update(balances);
        self.reserves.set(&reserves);

        // Calculate the amounts provided by user (i.e., the current
        // balances minus the previously-recorded reserve balances).
        let amounts = [
            balances[0].strict_sub(prev_balances[0].into()),
            balances[1].strict_sub(prev_balances[1].into()),
        ];

        ensure(
            amounts[0] > 0 && amounts[1] > 0,
            ISwapV2PairError::InsufficientLiquidityMinted,
        );

        let lp = self.lp();
        let lp_total_supply = lp.total_supply();

        let lp_to_create = if lp_total_supply == 0 {
            // Permanently lock the first MINIMUM_LIQUIDITY tokens
            lp.mint(&Self::BLACK_HOLE, Self::MINIMUM_LIQUIDITY);

            // TODO: multiply in u256 to allow amounts up to u128::MAX
            amounts[0]
                .strict_mul(amounts[1])
                .isqrt()
                .strict_sub(Self::MINIMUM_LIQUIDITY)
        } else {
            min(
                amounts[0]
                    .strict_mul(lp_total_supply)
                    .strict_div(prev_balances[0].into()),
                amounts[1]
                    .strict_mul(lp_total_supply)
                    .strict_div(prev_balances[1].into()),
            )
        };

        ensure(
            lp_to_create > 0,
            ISwapV2PairError::InsufficientLiquidityMinted,
        );

        // Mint the new liquidity to the address specified by the LP
        lp.mint(to, lp_to_create);
        let lp_minted = lp_to_create;

        log_event(&MintEvent::new(&get_caller(), lp_minted));

        // Return the amount of liquidity minted
        lp_minted
    }

    // Return pair-tokens in exchange for LP tokens that have been transferred
    // to us
    fn burn_lp(&mut self, to: &Address) -> Amounts {
        let balances = self._get_token_balances();

        let lp = self.lp();

        // Amount of LP tokens returned to us
        let lp_returned = lp.get_balance();

        let total_liquidity = lp.total_supply();

        // NOTE: added this check to avoid divide-by-zero below
        ensure(total_liquidity > 0, ISwapV2PairError::NoLiquidityToBurn);

        let amounts = [
            lp_returned.strict_mul(balances[0]) / total_liquidity,
            lp_returned.strict_mul(balances[1]) / total_liquidity,
        ];
        ensure(
            amounts[0] > 0 && amounts[1] > 0,
            ISwapV2PairError::InsufficientLiquidityBurned,
        );

        // Vaporize our LP tokens
        lp.burn(&get_current_address(), lp_returned);

        // Return pair tokens to the user
        self._transfer_to(to, amounts);

        // Update the record of our holdings
        self.sync();

        log_event(&BurnEvent::new(&get_caller(), amounts, to));

        // Return amounts redeemed
        amounts
    }

    fn swap(
        &mut self,
        amount0_out: Amount,
        amount1_out: Amount,
        to: &Address,
    ) {
        let amounts_out = [amount0_out, amount1_out];

        ensure(
            amounts_out[0] > 0 || amounts_out[1] > 0,
            ISwapV2PairError::InsufficientOutputAmount,
        );

        // Get the previous reserve balances
        let mut reserves = self.reserves.get();
        let r = reserves.balances();

        ensure(
            amounts_out[0] < r[0].into() && amounts_out[1] < r[1].into(),
            ISwapV2PairError::InsufficientLiquidity,
        );

        let addrs = self.token_addrs();
        ensure(
            to != &addrs[0] && to != &addrs[1],
            ISwapV2PairError::InvalidDestination,
        );

        self._transfer_to(to, amounts_out);

        // Optional TODO: Callback for flash-swap goes here.
        // (Safety: This method must be non-reentrant.)

        // Update the snapshot of reserve balances
        let balances = self._get_token_balances();
        reserves.update(balances);
        self.reserves.set(&reserves);

        // The unchecked subtractions here will not wrap, due to an `ensure` above
        let amounts_in = [
            // cur - (prev - out)
            balances[0].saturating_sub(Amount::from(r[0]) - amounts_out[0]),
            balances[1].saturating_sub(Amount::from(r[1]) - amounts_out[1]),
        ];

        ensure(
            amounts_in[0] > 0 || amounts_in[1] > 0,
            ISwapV2PairError::InsufficientInputAmount,
        );

        let balance_adj = [
            balances[0].strict_mul(1000).strict_sub(amounts_in[0] * 3),
            balances[1].strict_mul(1000).strict_sub(amounts_in[1] * 3),
        ];

        let balance_prod =
            U256::from(balance_adj[0]).mul(U256::from(balance_adj[1]));

        let reserve_adj = [u128::from(r[0]) * 1000, u128::from(r[1]) * 1000];

        let reserve_prod =
            U256::from(reserve_adj[0]).mul(U256::from(reserve_adj[1]));

        ensure(
            balance_prod >= reserve_prod,
            ISwapV2PairError::InvalidSwapProduct,
        );

        log_event(&SwapEvent::new(&get_caller(), amounts_in, amounts_out, to));
    }

    // Extract any reserve balances surplus to what was last recorded
    fn skim(&mut self, to: &Address) {
        let reserves = self.reserves.get();
        let prev_balances = reserves.balances();

        let cur_balances = self._get_token_balances();

        let surpluses = [
            cur_balances[0].strict_sub(prev_balances[0].into()),
            cur_balances[1].strict_sub(prev_balances[1].into()),
        ];

        self._transfer_to(to, surpluses);
    }

    // Record current reserve balances
    fn sync(&mut self) {
        let balances = self._get_token_balances();
        self.reserves.load_mut().update(balances);
    }

    // Driver helpers

    fn handle_purchase(&mut self, purchaser: &ContractAddress, amounts_in: Amounts, recipient: &Address) {
        // Transfer in the purchaser's A/B tokens
        self.transfer_amounts_from(purchaser, amounts_in);

        let _minted = self.mint_lp(recipient);
    }

    fn handle_redeem(&mut self, redeemer: &ContractAddress) {
        // Transfer in our allowances of the user's LP tokens
        self.transfer_lp_allowance_from(redeemer);

        let _redeemed = self.burn_lp(redeemer);
    }

    fn handle_swap(&mut self, trader: &ContractAddress, amounts_out: Amounts) {
        // Transfer in our allowances of the trader's A/B tokens
        self.transfer_allowances_from(trader);

        self.swap(amounts_out[0], amounts_out[1], trader);
    }
}

// Helper methods
impl SwapV2Pair {
    pub const MINIMUM_LIQUIDITY: u128 = 1000;
    pub const BLACK_HOLE: Address = Address::zero();

    fn lp(&self) -> ERC20Client {
        ERC20Client::new(&self.lp_addr.get())
    }

    fn token_addrs(&self) -> [Address; 2] {
        self.tokens.get()
    }

    fn tokens(&self) -> [ERC20Client; 2] {
        self.token_addrs().map(|ref addr| ERC20Client::new(addr))
    }

    fn _mint_fee(&mut self) -> bool {
        // Optional TODO: support fee_to setting

        false
    }

    // Get our holdings of the A/B tokens
    fn _get_token_balances(&self) -> Amounts {
        self.tokens().map(|t| t.get_balance())
    }

    fn _transfer_to(&mut self, to: &Address, amounts: Amounts) {
        self._try_transfer_to(to, amounts)
            .unwrap_or_else(|e| throw(e));
    }

    fn _try_transfer_to(
        &mut self,
        recipient: &Address,
        amounts: Amounts,
    ) -> Result<(), ISwapV2PairError> {
        for (token, amount) in zip(self.tokens(), amounts) {
            token
                .try_transfer(recipient, amount)
                .map_err(|_| ISwapV2PairError::TransferFailed)?;
        }

        Ok(())
    }

    fn _transfer_from(&mut self, from: &Address, amounts: Amounts) {
        self._try_transfer_from(from, amounts)
            .unwrap_or_else(|e| throw(e));
    }

    fn _try_transfer_from(
        &mut self,
        from: &Address,
        amounts: Amounts,
    ) -> Result<(), ISwapV2PairError> {
        let me = get_current_address();

        for (token, amount) in zip(self.tokens(), amounts) {
            token
                .try_transfer_from(from, &me, amount)
                .map_err(|_| ISwapV2PairError::TransferFailed)?;
        }

        Ok(())
    }

    // Helpers for driver routines

    fn transfer_amounts_from(&self, owner: &Address, amounts: Amounts) {
        let this = get_current_address();
        let tokens = self.tokens();

        for i in 0..=1 {
            tokens[i].transfer_from(owner, &this, amounts[i]);
        }
    }

    fn transfer_allowances_from(&self, owner: &Address) {
        let this = get_current_address();

        for t in self.tokens() {
            // We are allowed to spend this much on behalf of the owner
            let amount = t.allowance(owner, &this);

            if amount > 0 {
                // Transfer it all to us
                t.transfer_from(owner, &this, amount);
            }
        }
    }

    fn transfer_lp_allowance_from(&self, owner: &Address) {
        let this = get_current_address();

        // We are allowed to spend this much on behalf of the owner
        let amount = self.lp().allowance(owner, &this);

        // Transfer it all to us
        self.lp().transfer_from(owner, &this, amount);
    }
}
