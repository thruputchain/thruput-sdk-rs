//! Test one contract calling the other, both running as rv32 guests.
//!
//! Requires the lowered artifacts of the `erc20-rv32` and `caller-rv32`
//! wrapper packages: `scripts/dev.sh build-examples-rv32` writes them.
mod common;
use common::*;
use sdk::Named;

// These paths only work for the example-contracts directory
// that is hosted in the SDK package.
//
const ERC20_PATH: &str = "../../target/groundhog/erc20_rv32.rv32a";
const CALLER_PATH: &str = "../../target/groundhog/caller_rv32.rv32a";

#[test]
fn caller_test_rv32() {
    let erc20_contract = sdk::load_rv32_contract(ERC20_PATH);
    let caller_contract = sdk::load_rv32_contract(CALLER_PATH);

    let env = sdk::TestEnvironment::no_cost();

    let erc20_hash = env.setup_create_contract(&erc20_contract);
    let caller_hash = env.setup_create_contract(&caller_contract);

    env.transact(|tx| {
        tx.call(&sdk::ContractAddress::named("test")).run(move || {
            caller_test_inner(&erc20_hash, &caller_hash);
        })
    })
    .assert_success();
}
