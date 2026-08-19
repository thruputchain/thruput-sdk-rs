//! Regression fixture: method specs are sourced from the `#[contract]`
//! *trait*, never from the impl's own signatures.
//!
//! An impl may legitimately rename a parameter it does not use — the
//! convention is a leading underscore — and the published interface spec is
//! a contract's public API, so it must keep the trait's names. `Pair` names
//! its parameters `factory`, `numerator` and `denominator`; the impl below
//! renames every one of them.
//!
//! Trait and impl live in the SAME crate deliberately. Both emit `Method`
//! fragments for the interface, and impl-sourced signatures would make the
//! two disagree — so a regression shows up either as a build failure
//! (`conflicting method definitions` out of canonicalization) or as
//! `_factory` reaching the spec, which
//! `check_rv32_contract_spec_artifacts` asserts against.
//!
//! Replaces the guard the deleted `swap` contract carried, which was the
//! only end-to-end check of this invariant.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::*;

#[contract]
pub trait Pair {
    fn initialize(factory: &ContractAddress);
    fn set_rate(numerator: u64, denominator: u64) -> u64;
}

pub struct TraitSourcedMethodSpecContract;

#[contract]
impl Pair for TraitSourcedMethodSpecContract {
    // Every parameter is renamed. Nothing here reaches the spec: the
    // interface's argument names and order come from `Pair` above.
    fn initialize(_factory: &ContractAddress) {}

    fn set_rate(_numerator: u64, _denominator: u64) -> u64 {
        _denominator.wrapping_add(_numerator)
    }
}
