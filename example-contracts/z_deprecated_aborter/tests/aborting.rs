use sdk::*;

// See simpler abort tests in tests/aborts.rs

#[test]
fn test_abort_exn() {
    let mut env = TestEnvironment::default();

    let test_contract = aborter::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_exec(env, move || {
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
    })
    .unwrap_or_else(|e| panic!("Unexpected outer harness failure: {e}"));
}

#[test]
fn test_uncaught_abort() {
    let mut env = TestEnvironment::default();

    let test_contract = aborter::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_exec(env, move || {
        let nonce = SHA256Hash::from([7; 32]);
        let deployed_addr = deploy_contract(&nonce, &contract_hash).unwrap();
        let mut client = aborter::ContractClient::new(&deployed_addr);

        // We are not yet trapping aborts in the method wrappers, so here's
        // what happens now:
        // - The client method calls `abort`
        // - `abort` finds no handlers, so panics "Uncaught abort!"
        // - our invoke syscall here receives `InvokeError::Syscall`
        //
        match client.uncaught_abort() {
            Err(InvokeError::Syscall(_)) => { /* Expected */ }
            Err(a @ InvokeError::AbortedWith(_)) => {
                // Once the client behaves properly (calls `contract_abort` instead of trapping),
                // we should get here.  For now:
                //
                panic!("Unexpected abort: {a:?}");
            }
            Err(e) => panic!("Unexpected invoke error: {:?}", e),
            Ok(()) => panic!("Contract unexpectedly succeeded"),
        }
    })
    .unwrap_or_else(|e| panic!("Unexpected outer harness failure: {e}"));
}
