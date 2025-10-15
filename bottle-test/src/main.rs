use sdk::contract_test::load_wasm_contract;
use sdk::*;
use bottle_test_if::*;

pub fn main() {
    let package = std::env::args()
        .skip(1)
        .next()
        .unwrap_or_else(|| panic!("Please specify contract package to test"));

    bottle_test(&package);
}

fn bottle_test(package: &str) {
    let target_contract =
        load_wasm_contract(format!("target/optimized/{package}.wasm"));

    let tester_contract =
        load_wasm_contract(format!("target/bottle-test/{package}.wasm"));

    let env = TestEnvironment::new();

    // Create the target contract
    let target_hash = env.setup_create_contract(&target_contract);

    // Create and deploy the tester contract
    let tester_hash = env.setup_create_contract(&tester_contract);

    // Something we can move into the test closure
    let package = package.to_string();

    let mut handler = env.prepare_test(move || -> () {

        let nonce: SHA256Hash = [7; 32].into();
        let tester_addr = deploy_contract(&nonce, &tester_hash).unwrap();

        let mut tester = BottleTestClient::new(&tester_addr);

        tester.setup(&target_hash).unwrap_or_else(|e|
            panic!("BottleTest::setup() failed: {}", e));

        let gas_start = stats::gas_remaining();

        tester.run().unwrap_or_else(|e|
            panic!("BottleTest::run() failed: {}", e));

        let gas_end = stats::gas_remaining();

        let gas_consumed = gas_start.checked_sub(gas_end).unwrap_or_else(||
            panic!("Negative gas consumed!"));

        eprintln!("Bottle test succeeded (gas consumed: {})", gas_consumed);

        // Emit metrics
        println!("{}\tgas_consumed\t{}", package, gas_consumed);
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
}
