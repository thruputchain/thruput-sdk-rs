//! Requires the lowered artifact of the `exceptions-rv32` wrapper package:
//! `scripts/dev.sh build-examples-rv32` writes it.

// test working directory is example contract directory
const RV32_PATH: &str = "../../target/groundhog/exceptions_rv32.rv32a";

mod common;
use common::*;
use sdk::*;

#[test]
fn rv32_ok() {
    let contract = sdk::load_rv32_contract(RV32_PATH);
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
    let contract = sdk::load_rv32_contract(RV32_PATH);
    let env = sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let _addr = sdk::deploy_contract(&TEST_NONCE, &contract_hash);
        })
    })
    .assert_success();
}

#[test]
fn rv32_exceptions() {
    let contract = sdk::load_rv32_contract(RV32_PATH);
    let env = sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || exceptions_test(&contract_hash))
    })
    .assert_success();
}

#[test]
fn rv32_panic() {
    let contract = sdk::load_rv32_contract(RV32_PATH);
    let env = sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || panic_test(&contract_hash))
    })
    .assert_success();
}
