pub use erc20_test::*;

use sdk::{ContractAddress, Named, SHA256Hash, TestEnvironmentHandle};

pub fn test_erc20_contract(
    env: TestEnvironmentHandle,
    test_contract_hash: &SHA256Hash,
) {
    let test_contract_hash = *test_contract_hash;

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("erc20_test")).run(move || {
            let deployed_addr = erc20_test_setup(&test_contract_hash);
            let model = erc20_test(&deployed_addr);
            // The model itself isn't XDR-able; its expected event log is,
            // and it's all the post-commit check needs.
            model.events
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

    // Check that the event log matches the model
    let expected: Vec<groundhog_erc20_if::StdERC20Event> =
        result.assert_return_as();
    check_events(&expected, result.events());
}
