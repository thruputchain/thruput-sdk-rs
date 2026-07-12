use codegen_hygiene::HygieneClient;
use sdk::*;

static NONCE: SHA256Hash = sdk::sha256_static!("CODEGEN_HYGIENE_NONCE");

#[test]
fn params_named_after_generated_locals_pass_through() {
    let env = sdk::TestEnvironment::no_cost();
    let contract = codegen_hygiene::register_mock_contract(&env);
    let hash = contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&NONCE, &hash);
            let client = HygieneClient::new(&addr);

            assert_eq!(client.echo_entrypoint(42), 42);
            assert_eq!(client.echo_args(43), 43);
            assert_eq!(client.echo_state(44), 44);
            assert_eq!(client.echo_state_node(45), 45);
            assert_eq!(client.echo_sum(1, 2, 3), 6);

            // try_ variants generate their own bindings — exercise one.
            assert_eq!(client.try_echo_entrypoint(46).unwrap(), 46);
        })
    })
    .assert_success();
}
