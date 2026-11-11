mod common;
use thruput_sdk::Named;

fn run(check: fn(&ristretto_msm::RistrettoMsmClient)) {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let hash = *ristretto_msm::register_mock_contract(&env).as_hash();
    env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || common::run(&hash, check))
    })
    .assert_success();
}

#[test]
fn arithmetic() {
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
