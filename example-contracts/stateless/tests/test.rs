use stateless::*;

#[test]
fn test_run() {
    sdk::client_test(
        |env| env.register_mock(Stateless::__mock),
        |c: &StatelessClient| {
            c.run();
        },
    );
}

use stateless::Stateless;
use stateless::interface1;
use stateless::interface2;

#[test]
fn test_ifaces() {
    sdk::client_test(
        |env| env.register_mock(<Stateless as interface1::Interface>::__mock),
        |c: &interface1::Client| {
            assert_eq!(c.perform_task(), 1);
        },
    );

    sdk::client_test(
        |env| env.register_mock(<Stateless as interface2::Interface>::__mock),
        |c: &interface2::Client| {
            assert_eq!(c.perform_task(), 2);
        },
    );
}

#[test]
fn test_register_mock_single_trait() {
    let env = sdk::TestEnvironment::no_cost();
    let contract = env.register_mock(Stateless::__mock);
    let addr: sdk::ContractAddress = [0x11; 32].into();

    contract.deploy_contract_to(&addr);

    sdk::contract_exec(env, move || {
        let client = StatelessClient::new(&addr);
        client.run();
    })
    .unwrap_or_else(|e| {
        panic!("register_mock single-trait test failed: {:?}", e)
    });
}

#[test]
fn test_register_mock_multi_trait() {
    let env = sdk::TestEnvironment::no_cost();
    let contract = env.register_mock(|c| {
        <Stateless as interface1::Interface>::__mock(c);
        <Stateless as interface2::Interface>::__mock(c);
    });
    let addr: sdk::ContractAddress = [0x22; 32].into();

    contract.deploy_contract_to(&addr);

    sdk::contract_exec(env, move || {
        let client1 = interface1::Client::new(&addr);
        let client2 = interface2::Client::new(&addr);

        assert_eq!(client1.perform_task(), 1);
        assert_eq!(client2.perform_task(), 2);
    })
    .unwrap_or_else(|e| {
        panic!("register_mock multi-trait test failed: {:?}", e)
    });
}

#[test]
fn test_register_mock_package_all_exports() {
    let env = sdk::TestEnvironment::no_cost();
    let contract = sdk::register_mock_package(&env, env!("CARGO_PKG_NAME"));
    let addr: sdk::ContractAddress = [0x33; 32].into();

    contract.deploy_contract_to(&addr);

    sdk::contract_exec(env, move || {
        let stateless = StatelessClient::new(&addr);
        let client1 = interface1::Client::new(&addr);
        let client2 = interface2::Client::new(&addr);

        stateless.run();
        assert_eq!(client1.perform_task(), 1);
        assert_eq!(client2.perform_task(), 2);
    })
    .unwrap_or_else(|e| {
        panic!("register_mock package test failed: {:?}", e)
    });
}
