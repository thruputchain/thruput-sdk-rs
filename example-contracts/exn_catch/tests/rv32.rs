//! Requires the lowered `exn_catch` artifact: `scripts/dev.sh
//! build-examples-rv32` writes it.

// test working directory is example contract directory
const RV32_PATH: &str = "../../target/groundhog/exn_catch.rv32a";

mod common;
use common::*;
use sdk::*;

#[test]
fn rv32_exn_catch() {
    let contract = sdk::load_rv32_contract(RV32_PATH);
    let env = sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || exn_catch_test(&contract_hash))
    })
    .assert_success();
}
