use sdk::*;

#[test]
fn test_abort_raw() {
    let env = TestEnvironment::default(None);

    let result = contract_test_raw(env, || {
        contract_abort(&[1u8, 2, 3]);
    });

    match result {
        Err(e @ CallError::Abort(buf)) => {
            assert_eq!(buf.as_slice(), &[1, 2, 3]);

            println!("Aborted as expected: {e}")
        }
        r => panic!("Unexpected invocation result: {:?}", r),
    }
}

#[test]
fn test_abort_exn() {
    let mut env = TestEnvironment::default(None);

    let test_contract = aborter::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_exec(env, move || {
        let nonce = SHA256Hash::from([7; 32]);
        let deployed_addr = deploy_contract(&nonce, &contract_hash).unwrap();
        let mut client = aborter::ContractClient::new(&deployed_addr);

        // Call a client method that catches a syscall exception with an abort-handler
        // and returns it via `contract_abort`.
        match client.try_bad_bytestring_length() {
            Err(CallError::Abort(_)) => {
                // Good: we expect an abort with the syscall exn
            }
            r => panic!("Unexpected client result: {r:?}"),
        }
    })
    .unwrap_or_else(|e| panic!("Unexpected outer harness failure: {e}"));
}

#[test]
fn test_uncaught_abort() {
    let mut env = TestEnvironment::default(None);

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
