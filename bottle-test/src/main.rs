use bottle_test_if::*;
use sdk::deploy_address;
use sdk::load_wasm_contract;
use sdk::transaction::InvocationFrameStatusCode;
use sdk::*;

pub fn main() {
    let package = std::env::args()
        .skip(1)
        .next()
        .unwrap_or_else(|| panic!("Please specify contract package to test"));

    bottle_test(&package);
}

pub const TARGET_NONCE: SHA256Hash = SHA256Hash::new([7; 32]);
pub const TESTER_NONCE: SHA256Hash = SHA256Hash::new([7; 32]);
pub const OWNER: ContractAddress = ContractAddress::new([0xFF; 32]);

fn bottle_test(package: &str) {
    let target_contract =
        load_wasm_contract(format!("target/optimized/{package}.wasm"));

    let tester_contract =
        load_wasm_contract(format!("target/bottle-test/{package}.wasm"));

    let env = TestEnvironment::no_cost();

    // Create and deploy the tester contract
    let tester_hash = env.setup_create_contract(&tester_contract);
    let tester_addr = deploy_address(&OWNER, &TESTER_NONCE, &tester_hash);
    // env.setup_grant_storage_tokens(&tester_addr, 1_000_000_000);

    // Create the target contract
    let target_hash = env.setup_create_contract(&target_contract);
    let _target_addr =
        deploy_address(&tester_addr, &TARGET_NONCE, &target_hash);
    // env.setup_grant_storage_tokens(&target_addr, 1_000_000_000);

    // Something we can move into the test closure
    let package = package.to_string();

    let mut handler = contract_test_handler(env, move || -> () {
        let tester_addr = deploy_contract(&TESTER_NONCE, &tester_hash);

        let mut tester = BottleTestClient::new(&tester_addr);

        tester.setup(&target_hash);

        let gas_start = stats::gas_remaining();

        tester.run();

        let gas_end = stats::gas_remaining();

        let gas_consumed = gas_start
            .checked_sub(gas_end)
            .unwrap_or_else(|| panic!("Negative gas consumed!"));

        eprintln!("Bottle test succeeded (gas consumed: {})", gas_consumed);

        // Emit metrics
        println!("{}\tgas_consumed\t{}", package, gas_consumed);
    });

    let res = handler.root_invoke();
    if res.output.status_code() != InvocationFrameStatusCode::Return {
        panic!(
            "Bottle-test invocation failed with code {}",
            res.output.status_code()
        );
    }

    let eres = handler.try_commit_transaction();
    if eres != sdk::ExecResult::ExecResultOK {
        panic!("Bottle-test transaction failed: {eres:?}");
    }

    // Print out the transaction's event log
    {
        use groundhog_erc20_if::*;
        use sdk::EmittedEvent;

        for (i, event) in res.events.iter().enumerate() {
            let event: EmittedEvent = event.into();
            match ERC20Event::try_from(event) {
                Ok(event) => eprintln!("\tevent {:?}: {}", i, event),
                Err(_) => eprintln!("\tevent {:?}: <non-ERC20 event>", i),
            }
        }
    }
}
