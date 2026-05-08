//! A request router for the example contracts `swap` and `swap_driver`

#![allow(clippy::result_large_err)]

use crate::cfg::*;
use sdk::storage::*;
use sdk::*;
use swap::*;
use groundhog_erc20_permit_if::*;

#[storage]
pub struct Router {
    // Configuration
    cfg: Value<Cfg>,
}

const A: usize = 0;
const B: usize = 1;

#[contract(throws(ERC20Error, ISwapV2PairError))]
impl Router {
    // Initialize router with knowledge of a single swap pair.
    //
    // (A more complete system would use a swap-pair-factory contract to route
    // to arbitrary swap pairs.)
    //
    pub fn initialize(
        &mut self,
        driver: &Address,
        swap_pair: &Address,
        token0: &Address,
        token1: &Address,
    ) {
        if self.cfg.try_get().is_some() {
            panic!("Already initialized.");
        }

        let cfg = Cfg {
            driver: *driver,
            swap_pair: *swap_pair,
            token0: *token0,
            token1: *token1,
        };

        self.cfg.set(&cfg);
    }

    pub fn succeeds() {}

    pub fn fails() {
        panic!("Uh oh!");
    }

    #[allow(unreachable_code, unused_variables)]
    pub fn add_liquidity(
        &self,
        token_a: &Address,
        token_b: &Address,
        amounts_desired: Amounts,
        amounts_min: Amounts,
        recipient: &Address,
        permits: [Permit; 2],
    ) {
        let cfg = self.cfg.get();

        let (pair, driver) = cfg.get_pair(token_a, token_b);

        let (reserve_a, reserve_b, _) = pair.try_get_reserves().unwrap();

        let amounts: Amounts = if reserve_a == 0 && reserve_b == 0 {
            amounts_desired
        } else {
            let amount_optimal_b = quote(amounts_desired[A], [reserve_a, reserve_b]);

            if amount_optimal_b <= amounts_desired[B] {
                assert!(amount_optimal_b >= amounts_min[B], "UniswapV2Router: INSUFFICIENT_B_AMOUNT");
                [amounts_desired[A], amount_optimal_b]
            } else {
                let amount_optimal_a = quote(amounts_desired[B], [reserve_b, reserve_a]);
                assert!(amount_optimal_a <= amounts_desired[A]);
                assert!(amount_optimal_a >= amounts_min[A], "UniswapV2Router: INSUFFICIENT_A_AMOUNT");
                [amount_optimal_a, amounts_desired[B]]
            }
        };

        // In the standard flow, we'd now transfer the amounts (which should
        // be within allowances set by the caller) to the pair:
        //
        //   self.token0()
        //     .transfer(&cfg.pair_addr, amounts[0]);
        //   self.token1()
        //     .transfer(&cfg.pair_addr, amounts[1]);
        //
        // For our batched flow, we'll instead just forward the permits to the
        // driver.

        let purchaser = get_caller();
        driver.try_request_purchase_with_permits(&purchaser, amounts, recipient, permits).unwrap();
    }
}

// adapted from UniswapV2Library
/// Given an amount of token X, give a price quote for token Y
fn quote(amount_x: Amount, reserves: Amounts) -> Amount {
    const X: usize = 0;
    const Y: usize = 1;

    assert!(amount_x > 0, "Insufficient amount");

    let amount_y = (U256::from(amount_x) * U256::from(reserves[Y])) / U256::from(reserves[X]);

    amount_y.try_into().unwrap()
}
