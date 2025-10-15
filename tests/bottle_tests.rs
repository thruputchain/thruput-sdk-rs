use sdk::contract_test::*;
use bottle_test_if::*;

#[test]
fn bottle_test() {
    let package = "erc20_no_storage";
    let target_contract = load_wasm_contract(format!("target/optimized/{package}.wasm"));
    let tester_contract = load_wasm_contract(format!("target/bottle-test/{package}.wasm"));

    let env = sdk::TestEnvironment::new();
    let target_hash = env.setup_create_contract(&target_contract);
    let tester_hash = env.setup_create_contract(&tester_contract);

    let mut handler = env.prepare_test(move || -> () {
        use sdk::*;

        let tester_nonce = SHA256Hash::from([8; 32]);
        let tester_addr = deploy_contract(&tester_nonce, &tester_hash).unwrap();

        let mut tester = BottleTestClient::new(&tester_addr);
        tester.run(&target_hash).unwrap_or_else(|e|
            panic!("tester.run() failed: {}", e));
    });

    let res = handler.root_invoke();
    if res.result.panic != sdk::InvokeErrorCode::kInvokeNone as i32 {
        panic!("root invocation panicked with code {}", res.result.panic);
    }
}
