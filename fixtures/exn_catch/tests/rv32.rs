//! Requires the lowered `exn_catch` artifact: `cargo gnaw build exn_catch`
//! writes it.

// test working directory is example contract directory
const RV32_PATH: &str = "../../target/thruput/exn_catch.rv32a";

mod common;
use thruput_sdk::*;

#[test]
fn rv32_exn_catch() {
    run(common::exn_catch_test);
}

#[test]
fn rv32_exn_catch_mapped() {
    run(common::exn_catch_mapped_test);
}

fn run(test: fn(&CodeHash<exn_catch::ExnCatchClient>)) {
    let path = std::env::var("EXN_CATCH_RV32_ARTIFACT")
        .unwrap_or_else(|_| RV32_PATH.into());
    let contract = thruput_sdk::load_rv32_contract(&path);
    let env = thruput_sdk::TestEnvironment::no_cost();
    let code = CodeHash::new(env.setup_create_contract(&contract));

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || test(&code))
    })
    .assert_success();
}
