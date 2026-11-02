use stateless::*;
use thruput_sdk::Named;

#[test]
fn test_run() {
    thruput_sdk::client_test(
        |env| stateless::register_mock_contract(env),
        |c: &StatelessClient| {
            c.run();
        },
    );
}

use stateless::interface1;
use stateless::interface2;

#[test]
fn test_ifaces() {
    thruput_sdk::client_test(
        |env| stateless::register_mock_contract(env),
        |c: &interface1::Client| {
            assert_eq!(c.perform_task(), 1);
        },
    );

    thruput_sdk::client_test(
        |env| stateless::register_mock_contract(env),
        |c: &interface2::Client| {
            assert_eq!(c.perform_task(), 2);
        },
    );
}

#[test]
fn test_register_mock_contract_supports_direct_contract_client() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let contract = stateless::register_mock_contract(&env);
    let addr: thruput_sdk::ContractAddress = [0x11; 32].into();

    contract.deploy_contract_to(&addr);

    env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || {
                let client = StatelessClient::new(&addr);
                client.run();
            })
    })
    .assert_success();
}

#[test]
fn test_register_mock_contract_supports_all_exported_interfaces() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let contract = stateless::register_mock_contract(&env);
    let addr: thruput_sdk::ContractAddress = [0x22; 32].into();

    contract.deploy_contract_to(&addr);

    env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || {
                let client1 = interface1::Client::new(&addr);
                let client2 = interface2::Client::new(&addr);

                assert_eq!(client1.perform_task(), 1);
                assert_eq!(client2.perform_task(), 2);
            })
    })
    .assert_success();
}

#[test]
fn test_register_mock_contract_all_exports() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let contract = stateless::register_mock_contract(&env);
    let addr: thruput_sdk::ContractAddress = [0x33; 32].into();

    contract.deploy_contract_to(&addr);

    env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || {
                let stateless = StatelessClient::new(&addr);
                let client1 = interface1::Client::new(&addr);
                let client2 = interface2::Client::new(&addr);

                stateless.run();
                assert_eq!(client1.perform_task(), 1);
                assert_eq!(client2.perform_task(), 2);
            })
    })
    .assert_success();
}
