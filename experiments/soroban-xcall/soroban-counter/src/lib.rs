#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, symbol_short, Env, Symbol};

const COUNTER: Symbol = symbol_short!("COUNTER");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CounterError {
    Refused = 7,
}

#[contract]
pub struct Counter;

#[contractimpl]
impl Counter {
    pub fn increment(env: Env, by: u32) -> u32 {
        let count = Self::get(env.clone()) + by;
        env.storage().persistent().set(&COUNTER, &count);
        count
    }

    pub fn get(env: Env) -> u32 {
        env.storage().persistent().get(&COUNTER).unwrap_or(0)
    }

    /// Increments, then panics: the increment must be rolled back.
    pub fn inc_then_panic(env: Env, by: u32) -> u32 {
        Self::increment(env, by);
        panic!("boom")
    }

    /// Increments, then fails with a contract error: also rolled back.
    pub fn inc_then_err(env: Env, by: u32) -> Result<u32, CounterError> {
        Self::increment(env, by);
        Err(CounterError::Refused)
    }
}
