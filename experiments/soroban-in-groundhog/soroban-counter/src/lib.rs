#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Env, Symbol};

const COUNTER: Symbol = symbol_short!("COUNTER");

#[contract]
pub struct Counter;

#[contractimpl]
impl Counter {
    pub fn increment(env: Env, by: u32) -> u32 {
        let mut count: u32 = env.storage().persistent().get(&COUNTER).unwrap_or(0);
        count += by;
        env.storage().persistent().set(&COUNTER, &count);
        count
    }
}
