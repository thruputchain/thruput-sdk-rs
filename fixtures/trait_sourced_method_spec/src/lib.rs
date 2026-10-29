//! Regression fixture: method specs are sourced from the `#[contract]`
//! *trait*, never from the impl's own signatures.
//!
//! An impl may legitimately rename a parameter it does not use — the
//! convention is a leading underscore — and the published interface spec is
//! a contract's public API, so it must keep the trait's names. `Pair` names
//! its parameters `factory`, `numerator`, `denominator` and `value`; the impl below
//! renames every one of them.
//!
//! The impl emits method fragments from the trait's metadata. The extracted
//! RV32 spec must retain the trait's argument names and all three declared
//! effects, while leaving the unannotated method's effect absent.
//! gnaw's `fixtures:specs` task checks this after a real build.
//!
//! Replaces the guard the deleted `swap` contract carried, which was the
//! only end-to-end check of this invariant.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use thruput_sdk::*;

#[contract]
pub trait Pair {
    fn initialize(factory: &ContractAddress);
    #[contract(effect = "stateChanging")]
    fn set_rate(numerator: u64, denominator: u64) -> u64;
    #[contract(effect = "readOnly")]
    fn get_rate() -> u64;
    #[contract(effect = "pure")]
    fn calculate(value: u64) -> u64;
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

    fn get_rate() -> u64 {
        1
    }

    fn calculate(_value: u64) -> u64 {
        _value.wrapping_mul(2)
    }
}
