#![no_std]
//! Calls the token on someone's behalf -- the classic nested-auth case.
use soroban_sdk::{contract, contractimpl, vec, Address, Env, IntoVal, Symbol};

#[contract]
pub struct Router;

fn transfer(env: &Env, token: &Address, from: &Address, to: &Address, amount: i128) {
    let args = vec![env, from.into_val(env), to.into_val(env), amount.into_val(env)];
    env.invoke_contract::<()>(token, &Symbol::new(env, "transfer"), args);
}

#[contractimpl]
impl Router {
    /// `from` authorizes this call, and (as a sub-invocation) the transfer.
    pub fn pay(env: Env, from: Address, token: Address, to: Address, amount: i128) {
        from.require_auth();
        transfer(&env, &token, &from, &to, amount);
    }

    /// Transfers twice: needs two authorized transfer sub-invocations, one
    /// authorization must not be spent twice.
    pub fn pay_twice(env: Env, from: Address, token: Address, to: Address, amount: i128) {
        from.require_auth();
        transfer(&env, &token, &from, &to, amount);
        transfer(&env, &token, &from, &to, amount);
    }

    /// A malicious router: the user authorized `pay`-style arguments, but this
    /// sends the funds to `thief` instead of `to`.
    pub fn pay_evil(env: Env, from: Address, token: Address, to: Address, amount: i128) {
        from.require_auth();
        let _ = to;
        let thief = env.current_contract_address();
        transfer(&env, &token, &from, &thief, amount);
    }

    /// Spends the router's own balance: authorized because the router is the
    /// token's direct caller (Soroban's invoker-contract rule).
    pub fn pay_self(env: Env, token: Address, to: Address, amount: i128) {
        transfer(&env, &token, &env.current_contract_address(), &to, amount);
    }
}
