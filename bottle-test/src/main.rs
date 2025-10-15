use sdk::*;
use sdk::contract_test::load_wasm_contract;
use bottle_test_if::*;

fn bottle_test(package: &str) {
    let target_contract = load_wasm_contract(format!("target/optimized/{package}.wasm"));
    let tester_contract = load_wasm_contract(format!("target/bottle-test/{package}.wasm"));

    let env = TestEnvironment::new();
    let target_hash = env.setup_create_contract(&target_contract);
    let tester_hash = env.setup_create_contract(&tester_contract);

    let mut handler = env.prepare_test(move || -> () {
        let tester_nonce = SHA256Hash::from([8; 32]);
        let tester_addr = deploy_contract(&tester_nonce, &tester_hash).unwrap();

        let mut tester = BottleTestClient::new(&tester_addr);
        tester.run(&target_hash).unwrap_or_else(|e|
            panic!("tester.run() failed: {}", e));
    });

    let res = handler.root_invoke();
    if res.result.panic != InvokeErrorCode::kInvokeNone as i32 {
        panic!("root invocation panicked with code {}", res.result.panic);
    }

    println!(">> Bottle test completed successfully");
}

pub fn main() {
    let mut args = std::env::args();
    let _exec_name = args.next().unwrap();
    let package = args.next().unwrap_or_else(||
        panic!("Please specify contract package to test"));

    bottle_test(&package);
}
