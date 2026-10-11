use bottle_test_if::*;

use sdk::load_rv32_contract;
use sdk::*;

pub fn main() {
    let stem = std::env::args()
        .nth(1)
        .unwrap_or_else(|| panic!("Please specify contract artifact to test"));

    bottle_test(&stem);
}

pub const TESTER_NONCE: SHA256Hash = SHA256Hash::new([7; 32]);
pub const OWNER: ContractAddress = ContractAddress::new([0xFF; 32]);

/// `stem` is the artifact stem `scripts/dev.sh` staged the pair under: the
/// rv32 bin name of the contract's `-rv32` wrapper package. The two builds
/// differ only in the wrapper's `bottle-test` feature — the target is the
/// contract under measurement, the tester adds the `BottleTest` entrypoints
/// that drive it.
fn bottle_test(stem: &str) {
    let target_contract =
        load_rv32_contract(format!("target/bottle-test/{stem}.target.rv32a"));

    let tester_contract =
        load_rv32_contract(format!("target/bottle-test/{stem}.tester.rv32a"));

    let env = TestEnvironment::no_cost();

    // Create the tester and target contracts
    let tester_hash = env.setup_create_contract(&tester_contract);
    let target_hash = env.setup_create_contract(&target_contract);

    // Something we can move into the test closure
    let stem = stem.to_string();

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
            println!("{}\tgas_consumed\t{}", stem, gas_consumed);
        })
    });
    result.assert_success();

    // Print out the transaction's event log
    {
        use sdk::EmittedEvent;
        use thruput_erc20_if::*;

        for (i, event) in result.events().iter().enumerate() {
            let event: EmittedEvent = event.into();
            match ERC20Event::try_from(event) {
                Ok(event) => eprintln!("\tevent {:?}: {}", i, event),
                Err(_) => eprintln!("\tevent {:?}: <non-ERC20 event>", i),
            }
        }
    }
}
