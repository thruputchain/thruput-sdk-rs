//! Requires the lowered `exn_catch` artifact: `scripts/dev.sh
//! build-examples-rv32` writes it.

// test working directory is example contract directory
const RV32_PATH: &str = "../../target/thruput/exn_catch.rv32a";

mod common;
use common::*;
use thruput_sdk::*;

#[test]
fn rv32_exn_catch() {
    let path = std::env::var("EXN_CATCH_RV32_ARTIFACT")
        .unwrap_or_else(|_| RV32_PATH.into());
    let contract = thruput_sdk::load_rv32_contract(&path);
    let env = thruput_sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || exn_catch_test(&contract_hash))
    })
    .assert_success();
}
