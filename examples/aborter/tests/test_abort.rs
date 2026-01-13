use sdk::*;
use sdk::contract_test::*;
// use aborter::*;

#[test]
fn test_abort() {
    let env = TestEnvironment::new();

    let result = contract_test_invoke(env, || {
        contract_abort(&[]);
    });

    match result {
        Err(InvokeError::AbortedWith(_)) => println!("Aborted as expected"),
        r => panic!("Unexpected invocation result: {:?}", r)
    }
}

/*
fn test_inner(client: &mut ContractClient) -> InvokeResult<()> {
    let _ = with_debug_context!(&"Testing abort functionality", {
        with_abort_handler(&default_abort_handler, || {
            let _ = client.bytestring_failure();
            Ok(())
        })
    });

    contract_return(&[]);
}
*/

/*
#[test]
fn test_abort() {
    let mut env = sdk::TestEnvironment::new();

    let test_contract = test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    let mut handler = env.prepare_test(move || -> () {
        let nonce = SHA256Hash::from([7; 32]);
        let _deployed_addr = deploy_contract(&nonce, &contract_hash).unwrap();
        // let mut client = ContractClient::new(&deployed_addr);

        let buf = [0, 1, 2];
        contract_abort(&buf[..]);

        /*
        match test_inner(&mut client) {
            Ok(_) => panic!("Unexpected success"),
            Err(_) => (),
        }
        */
    });

    let res = handler.root_invoke();
    let status = res.status;

    if status != sdk::InvokeErrorCode::kInvokeNone {
        panic!("Root invocation handler returned {:?}", status);
    }

    let commit_res = handler.try_commit_transaction();

    if commit_res != ExecResultTxRootFrameReverted {
        panic!("Unexpected transaction result: {:?}", commit_res);
    }
}
*/
