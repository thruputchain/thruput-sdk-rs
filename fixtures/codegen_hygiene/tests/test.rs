use codegen_hygiene::HygieneClient;
use thruput_sdk::*;

static NONCE: DeployNonce =
    DeployNonce::new(thruput_sdk::sha256_static!("CODEGEN_HYGIENE_NONCE"));

#[test]
fn params_named_after_generated_locals_pass_through() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let contract = codegen_hygiene::register_mock_contract(&env);
    let code = CodeHash::<HygieneClient>::of_mock(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let client = code.deploy(&NONCE).client();

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
