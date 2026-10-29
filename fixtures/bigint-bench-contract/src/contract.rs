//! The contract wrapper around [`crate::kernel::run`].
//!
//! Stateless on purpose: no `#[storage]` root, so no storage syscall appears in
//! the measurement and no storage code appears in the artifact.

use crate::Selected;
use crate::backend::Backend;
use bigint_bench_if::BigIntBench;
use thruput_sdk::contract;

#[derive(Default)]
struct Bench;

#[contract]
impl BigIntBench for Bench {
    fn run(iters: u32, a_hi: u128, a_lo: u128, b_hi: u128, b_lo: u128) -> u64 {
        let a = Selected::from_hi_lo(a_hi, a_lo);
        let b = Selected::from_hi_lo(b_hi, b_lo);

        crate::kernel::run(iters, a, b).digest()
    }
}
