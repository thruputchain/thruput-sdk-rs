use crate::bignum::{U112, U256};
use crate::events::*;
use crate::interfaces::iswapv2_pair::*;
use sdk::*;

// The state value
//
// No concurrent sets allowed.
//
#[contract_type]
#[derive(Copy, Clone, Debug)]
pub struct Reserves {
    // The last recorded reserve balances.
    pub balances: [U112; 2],

    // Cumulative prices associated with the balances above.
    // (Consider calculating these dynamically instead of storing?)
    pub cumulative_prices: [U256; 2],

    // The block in which the reserve balances were last recorded.
    pub block_timestamp: u32,
}

impl Reserves {
    pub const fn new() -> Self {
        Self {
            balances: [U112::zero(), U112::zero()],
            cumulative_prices: [U256::zero(), U256::zero()],
            block_timestamp: 0,
        }
    }

    pub const fn balances(&self) -> [U112; 2] {
        self.balances
    }

    pub const fn block_timestamp(&self) -> u32 {
        self.block_timestamp
    }

    // Update the reserve balances, also updating the cumulative prices and
    // block timestamp.
    pub fn update(&mut self, balances: Amounts) {
        // If this fails, `skim()` may be used to extract the overflow
        ensure(
            balances[0] <= U112::MAX && balances[1] <= U112::MAX,
            ISwapV2PairError::Overflow,
        );

        // We intentionally truncate here, desiring the timestamp mod 2^32
        let current_block = get_block_timestamp() as u32;

        // Wrapping yields the right elapsed time if this method is
        // run at least once every ~139 years.
        let time_elapsed = current_block.wrapping_sub(self.block_timestamp);

        // Increment the cumulative prices
        let [r0, r1] = self.balances();
        if time_elapsed > 0 && r0 != 0 && r1 != 0 {
            self.cumulative_prices[0] +=
                uq112x112_div_mul(r1, r0, time_elapsed);
            self.cumulative_prices[1] +=
                uq112x112_div_mul(r0, r1, time_elapsed);
        }

        // This will not fail due to the checks at the top of this function
        self.balances = balances.map(|b| U112::try_from(b).unwrap());

        self.block_timestamp = current_block;

        log_event(&SyncEvent::new(balances));
    }
}

fn get_block_timestamp() -> u64 {
    get_block_height().into_inner().try_into().unwrap()
}

// Compute exactly: ((a * Q) / b) * p, where Q = 2^112
fn uq112x112_div_mul(a: U112, b: U112, p: u32) -> U256 {
    // TODO: accomplish this statically
    let ln_q: U256 = 112.into();

    let a: U256 = a.into();
    let b: U256 = b.into();
    let p: U256 = p.into();
    ((a << ln_q) / b) * p
}
