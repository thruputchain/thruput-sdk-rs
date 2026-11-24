use adder_v1::AdderClient as AdderV1Client;
use adder_v2::AdderClient as AdderV2Client;
use thruput_sdk::{CodeHash, ContractClient, DeployNonce, Named};

#[test]
fn both_versions_work_at_the_same_contract_address() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.2.0");
    assert_eq!(AdderV1Client::INTERFACE_VERSION, "0.1.0");
    assert_eq!(AdderV2Client::INTERFACE_VERSION, "0.2.0");

    let env = thruput_sdk::TestEnvironment::no_cost();
    let contract = versioned_impl::register_mock_contract(&env);
    let code = CodeHash::<(AdderV1Client, AdderV2Client)>::of_mock(&contract);
    let nonce = DeployNonce::new([7; 32].into());

    env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || {
                let address = code.deploy(&nonce);
                let v1 = AdderV1Client::at(&address);
                let v2 = AdderV2Client::at(&address);

                assert_eq!(v1.add_one(41), 42);
                // This value cannot be encoded by the 0.1 interface's u32 ABI.
                let large = u64::from(u32::MAX) + 41;
                assert_eq!(v2.add_one(large), large + 1);
                assert_eq!(v1.add_one(99), 100);
            })
    })
    .assert_success();
}
