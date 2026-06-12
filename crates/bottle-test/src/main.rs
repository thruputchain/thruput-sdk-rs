use bottle_test_if::*;

use sdk::load_wasm_contract;
use sdk::*;

pub fn main() {
    let package = std::env::args()
        .nth(1)
        .unwrap_or_else(|| panic!("Please specify contract package to test"));

    bottle_test(&package);
}

pub const TESTER_NONCE: SHA256Hash = SHA256Hash::new([7; 32]);
pub const OWNER: ContractAddress = ContractAddress::new([0xFF; 32]);

fn bottle_test(package: &str) {
    let target_contract =
        load_wasm_contract(format!("target/optimized/{package}.wasm"));

    let tester_contract =
        load_wasm_contract(format!("target/bottle-test/{package}.wasm"));

    let env = TestEnvironment::no_cost();

    // Create the tester and target contracts
    let tester_hash = env.setup_create_contract(&tester_contract);
    let target_hash = env.setup_create_contract(&target_contract);

    // Something we can move into the test closure
    let package = package.to_string();

    let result = env.transact(|tx| {
        tx.call(&OWNER).run(move || {
            let tester_addr = deploy_contract(&TESTER_NONCE, &tester_hash);

            let tester = BottleTestClient::new(&tester_addr);
            tester.setup(&target_hash);

            let gas_start = stats::gas_remaining();
            tester.run();
            let gas_end = stats::gas_remaining();

            let gas_consumed = gas_start
                .checked_sub(gas_end)
                .unwrap_or_else(|| panic!("Negative gas consumed!"));

            eprintln!(
                "Bottle test succeeded (gas consumed: {})",
                gas_consumed
            );
            println!("{}\tgas_consumed\t{}", package, gas_consumed);
        })
    });
    result.assert_success();

    // Print out the transaction's event log
    {
        use groundhog_erc20_if::*;
        use sdk::EmittedEvent;

        for (i, event) in result.events().iter().enumerate() {
            let event: EmittedEvent = event.into();
            match ERC20Event::try_from(event) {
                Ok(event) => eprintln!("\tevent {:?}: {}", i, event),
                Err(_) => eprintln!("\tevent {:?}: <non-ERC20 event>", i),
            }
        }
    }
}
