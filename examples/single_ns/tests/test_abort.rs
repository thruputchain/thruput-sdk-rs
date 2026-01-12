use sdk::*;
use sdk::abort;
use single_ns::*;

#[test]
fn test_abort() {
    let mut env = sdk::TestEnvironment::new();

    let test_contract = test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    let mut handler = env.prepare_test(move || -> () {
        let nonce = SHA256Hash::from([7; 32]);
        let deployed_addr = deploy_contract(&nonce, &contract_hash).unwrap();
        let mut client = ContractClient::new(&deployed_addr);

        test_inner(&mut client);
    });

    let res = handler.root_invoke();
    let status = res.status;

    if status != sdk::InvokeErrorCode::kInvokeNone {
        panic!("Root invocation handler returned {:?}", status);
    }
}

fn test_inner(_client: &mut ContractClient) {
    with_debug_context(&"Testing abort functionality", || {
        with_abort_handler(&default_abort_handler, || {
            abort(&"Hello!");
        })
    });
}
