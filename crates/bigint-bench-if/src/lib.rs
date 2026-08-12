//! Interface for the big-integer benchmark contract.
//!
//! One entrypoint, one operation per build. The contract that implements this
//! trait (`example-contracts/bigint-bench-contract`) is compiled once per
//! (backend, operation) pair, so the artifact under measurement contains
//! exactly one arithmetic kernel and nothing else. See
//! `docs/superpowers/specs/2026-08-12-bigint-bench-design.md`.
//!
//! Operands cross the ABI as four `u128` halves rather than a 256-bit contract
//! type on purpose: no big-integer implementation appears in the calldata path,
//! so a build measuring Parity's `uint` links no `sdk::U256` code at all (and
//! vice versa).

#![cfg_attr(not(feature = "dev"), no_std)]

use sdk::contract;

/// The benchmark kernel.
///
/// The explicit `version` decouples this interface's ABI version from the SDK
/// release version the crate inherits: the macro otherwise defaults a trait's
/// version to its crate's, which would force the implementing contract's
/// `#[contract(version = ...)]` to be edited on every SDK bump. Nothing outside
/// this repo binds to it, so it moves only when `run`'s signature does.
#[contract(client = BigIntBenchClient, version = "0.1.0")]
pub trait BigIntBench {
    /// Applies the compiled-in operation `iters` times and returns a digest of
    /// the final value.
    ///
    /// Both operands are reconstructed from `(hi, lo)` pairs, most-significant
    /// half first. The return value is a fold of every limb of the result: it
    /// exists so the loop cannot be dead-coded, not to be interpreted.
    ///
    /// Gas for one operation is `(gas(run) - gas(run @ op-none)) / iters`,
    /// measured on artifacts that differ only in which kernel they carry.
    fn run(iters: u32, a_hi: u128, a_lo: u128, b_hi: u128, b_lo: u128) -> u64;
}
