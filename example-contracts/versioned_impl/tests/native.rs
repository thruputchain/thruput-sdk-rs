use adder_v1::AdderClient as AdderV1Client;
use adder_v2::AdderClient as AdderV2Client;
use sdk::Named;

#[test]
fn both_versions_work_at_the_same_contract_address() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.2.0");
    assert_eq!(AdderV1Client::INTERFACE_VERSION, "0.1.0");
    assert_eq!(AdderV2Client::INTERFACE_VERSION, "0.2.0");

    let env = sdk::TestEnvironment::no_cost();
    let contract = versioned_impl::register_mock_contract(&env);
    let contract_hash = *contract.as_hash();
    let nonce: sdk::SHA256Hash = [7; 32].into();

    env.transact(|tx| {
        tx.call(&sdk::ContractAddress::named("test")).run(move || {
            let address = sdk::deploy_contract(&nonce, &contract_hash);
            let v1 = AdderV1Client::new(&address);
            let v2 = AdderV2Client::new(&address);

            assert_eq!(v1.add_one(41), 42);
            // This value cannot be encoded by the 0.1 interface's u32 ABI.
            let large = u64::from(u32::MAX) + 41;
            assert_eq!(v2.add_one(large), large + 1);
            assert_eq!(v1.add_one(99), 100);
        })
    })
    .assert_success();
}
