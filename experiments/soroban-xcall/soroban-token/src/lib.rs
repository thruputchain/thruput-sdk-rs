#![no_std]
//! Minimal token: `transfer` requires `from`'s authorization.
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    /// Test-only: unauthenticated mint.
    pub fn mint(env: Env, to: Address, amount: i128) {
        let b = Self::balance(env.clone(), to.clone());
        env.storage().persistent().set(&to, &(b + amount));
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        env.storage().persistent().get(&id).unwrap_or(0)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        let fb = Self::balance(env.clone(), from.clone());
        if amount < 0 || fb < amount {
            panic!("insufficient balance");
        }
        env.storage().persistent().set(&from, &(fb - amount));
        let tb = Self::balance(env.clone(), to.clone());
        env.storage().persistent().set(&to, &(tb + amount));
    }
}
