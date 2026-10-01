#![no_std]
//! Calls other contracts. Every call here leaves this Soroban contract's
//! Groundhog contract and becomes a Groundhog sub-invocation.
use soroban_sdk::{contract, contractimpl, vec, Address, Bytes, Env, IntoVal, Symbol, Val};

#[contract]
pub struct Caller;

fn counter_get(env: &Env, counter: &Address) -> u32 {
    env.invoke_contract(counter, &Symbol::new(env, "get"), vec![env])
}

#[contractimpl]
impl Caller {
    /// Soroban -> Soroban: `counter.increment(by)`.
    pub fn call_inc(env: Env, counter: Address, by: u32) -> u32 {
        env.invoke_contract(&counter, &Symbol::new(&env, "increment"), vec![&env, by.into_val(&env)])
    }

    /// `try_call` of a callee that increments then panics. Returns the
    /// counter's value afterwards: unchanged if the callee was rolled back.
    pub fn try_panic(env: Env, counter: Address, by: u32) -> u32 {
        let args = vec![&env, by.into_val(&env)];
        let r = env.try_invoke_contract::<u32, soroban_sdk::Error>(
            &counter, &Symbol::new(&env, "inc_then_panic"), args);
        assert!(r.is_err(), "callee should have failed");
        counter_get(&env, &counter)
    }

    /// Same, for a callee that returns a contract error. Returns
    /// (error code, counter value afterwards) as code * 1_000_000 + value.
    pub fn try_err(env: Env, counter: Address, by: u32) -> u32 {
        let args = vec![&env, by.into_val(&env)];
        let r = env.try_invoke_contract::<u32, soroban_sdk::Error>(
            &counter, &Symbol::new(&env, "inc_then_err"), args);
        let code = match r {
            Err(Ok(e)) => e.get_code(),
            _ => panic!("expected a contract error"),
        };
        code * 1_000_000 + counter_get(&env, &counter)
    }

    /// Soroban -> Groundhog: raw call of a native Groundhog contract
    /// (`method` = entrypoint id, `calldata` = its encoded args).
    pub fn gh_call(env: Env, target: Address, method: u64, calldata: Bytes) -> Bytes {
        let args: soroban_sdk::Vec<Val> = vec![&env, method.into_val(&env), calldata.into_val(&env)];
        env.invoke_contract(&target, &Symbol::new(&env, "gh_raw"), args)
    }
}
