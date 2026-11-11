mod common;
use thruput_sdk::Named;

fn run(check: fn(&ristretto_msm::RistrettoMsmClient)) {
    let path = std::env::var("RISTRETTO_MSM_RV32_ARTIFACT")
        .expect("set RISTRETTO_MSM_RV32_ARTIFACT to the experimental .rv32a");
    let contract = thruput_sdk::load_rv32_contract(&path);
    let env = thruput_sdk::TestEnvironment::no_cost();
    let hash = env.setup_create_contract(&contract);
    env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || common::run(&hash, check))
    })
    .assert_success();
}

#[test]
fn enabled_arithmetic() {
    run(common::arithmetic);
}

#[test]
fn malformed_is_error() {
    run(common::malformed);
}

#[test]
fn insufficient_gas_is_fault() {
    run(common::insufficient_gas);
}

#[test]
#[ignore = "requires an executor built with experimental MSM disabled"]
fn disabled_is_error() {
    run(common::disabled);
}
