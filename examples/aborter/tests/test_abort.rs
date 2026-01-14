use sdk::*;
use sdk::contract_test::*;

#[test]
fn test_abort_bare() {
    let env = TestEnvironment::new();

    let result = contract_test_invoke(env, || {
        contract_abort(&[1u8, 2, 3]);
    });

    match result {
        Err(e @ InvokeError::AbortedWith(3)) => println!("Aborted as expected: {e}"),
        r => panic!("Unexpected invocation result: {:?}", r)
    }
}

#[test]
fn test_abort_exn() {
    let mut env = TestEnvironment::new();

    let test_contract = aborter::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_test_invoke(env, move || {
        let nonce = SHA256Hash::from([7; 32]);
        let deployed_addr = deploy_contract(&nonce, &contract_hash).unwrap();
        let mut client = aborter::ContractClient::new(&deployed_addr);

        // Call a client method that catches a syscall exception with an abort-handler
        // and returns it via `contract_abort`.
        match client.bad_bytestring_length() {
            Err(InvokeError::AbortedWith(8)) => {
                // Good: we expect an abort with return-data of length 8 (the syscall exn)
            }
            r => panic!("Unexpected client result: {r:?}"),
        }
    }).unwrap_or_else(|e| panic!("Unexpected outer harness failure: {e}"));
}
