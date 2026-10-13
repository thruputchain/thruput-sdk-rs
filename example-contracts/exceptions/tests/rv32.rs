//! Requires the lowered `exceptions` artifact: `scripts/dev.sh
//! build-examples-rv32` writes it.

// test working directory is example contract directory
const RV32_PATH: &str = "../../target/thruput/exceptions.rv32a";

mod common;
use common::*;
use thruput_sdk::*;

#[test]
fn rv32_ok() {
    let contract = thruput_sdk::load_rv32_contract(RV32_PATH);
    let env = TestEnvironment::default();
    let _ = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(|| {
            // Do nothing
        })
    })
    .assert_success();
}

#[test]
fn rv32_deploy() {
    let contract = thruput_sdk::load_rv32_contract(RV32_PATH);
    let env = thruput_sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let _addr =
                thruput_sdk::deploy_contract(&TEST_NONCE, &contract_hash);
        })
    })
    .assert_success();
}

#[test]
fn rv32_exceptions() {
    let contract = thruput_sdk::load_rv32_contract(RV32_PATH);
    let env = thruput_sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || exceptions_test(&contract_hash))
    })
    .assert_success();
}

#[test]
fn rv32_panic() {
    let contract = thruput_sdk::load_rv32_contract(RV32_PATH);
    let env = thruput_sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || panic_test(&contract_hash))
    })
    .assert_success();
}
