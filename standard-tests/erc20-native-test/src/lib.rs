pub use erc20_test::*;

use sdk::hash_ident;
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

    print_events(&result);

    // Check that the event log matches the model
    let expected: Vec<groundhog_erc20_if::StdERC20Event> =
        result.assert_return_as();
    check_events(&expected, result.events());
}

// Print out a transaction's event log
fn print_events(result: &sdk::TransactResult) {
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

hash_ident! { static SPENDER: ContractAddress; }

/// Regression test: the native token precompile behaves like a standard
/// ERC20 contract for the operations it supports.
///
/// The precompile intentionally diverges from the reference contract (see
/// hogd's `precompile_native_token.cc`):
///
/// - `initialize`, `mint` and `burn` always fault: the supply is fixed at
///   genesis and there is no administrator.
/// - `total_supply` is the constant total HOG supply.
/// - An overdraft `transfer` is not rejected at invoke time with
///   [`groundhog_erc20_if::ERC20Error::TooLarge`]; the balance check is
///   deferred, so the transaction instead fails at commit.
///
/// Those divergences are asserted here too, so a change on either side
/// shows up as a regression.
pub fn test_native_token_precompile(env: TestEnvironmentHandle) {
    use groundhog_erc20_if::*;
    use sdk::precompiles::{
        ADDRESS_INITIAL_TOKEN_CLAIM, ADDRESS_PRECOMPILED_HOG_TOKEN,
    };

    let caller = ContractAddress::named("erc20_test");
    let token = &ADDRESS_PRECOMPILED_HOG_TOKEN;

    const BALANCE: Amount = 101;
    let total_supply: Amount =
        u128::from(groundhog_xdr::types::kTotalHogSupply)
            * u128::from(groundhog_xdr::types::kHogDivisibility);

    // Fund the test caller from the genesis allocation, which is held by
    // the initial-token-claim precompile.
    let result = env.transact(|tx| {
        tx.call(&ADDRESS_INITIAL_TOKEN_CLAIM).run(move || {
            ERC20Client::new(token).transfer(&caller, BALANCE);
        })
    });
    result.assert_success();
    env.advance_block();

    // Run the standard ERC20 test subset that applies to a pre-funded token
    // without mint/burn, and check the emitted events against the model.
    let result = env.transact(|tx| {
        tx.call(&caller).run(move || {
            let model = erc20_funded_test(token, BALANCE, total_supply);
            model.events
        })
    });
    result.assert_success();
    print_events(&result);
    let expected: Vec<StdERC20Event> = result.assert_return_as();
    check_events(&expected, result.events());
    env.advance_block();

    // transfer_from follows the standard error contract: an insufficient
    // allowance throws TooLarge, and a sufficient one is debited.
    let result = env.transact(|tx| {
        tx.call(&caller).run(move || {
            ERC20Client::new(token).adjust_allowance(&SPENDER, 5);
        })
    });
    result.assert_success();
    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&SPENDER).run(move || {
            let client = ERC20Client::new(token);
            let owner_balance = client.get_balance_of(&caller);
            let spender_balance = client.get_balance();

            assert!(matches!(
                client.try_transfer_from(&caller, &SPENDER, 6),
                Err(ERC20ClientError::ERC20Error(ERC20Error::TooLarge))
            ));

            client.transfer_from(&caller, &SPENDER, 5);
            assert_eq!(client.allowance(&caller, &SPENDER), 0);
            assert_eq!(client.get_balance_of(&caller), owner_balance - 5);
            assert_eq!(client.get_balance(), spender_balance + 5);
        })
    });
    result.assert_success();
    env.advance_block();

    // Divergence: initialize/mint/burn have no meaning for the native token
    // and always fault rather than throwing a typed ERC20Error.
    let result = env.transact(|tx| {
        tx.call(&caller).run(move || {
            ERC20Client::new(token).initialize(&caller);
        })
    });
    result.assert_fault();

    let result = env.transact(|tx| {
        tx.call(&caller).run(move || {
            ERC20Client::new(token).mint(&caller, 1);
        })
    });
    result.assert_fault();

    let result = env.transact(|tx| {
        tx.call(&caller).run(move || {
            ERC20Client::new(token).burn(&caller, 1);
        })
    });
    result.assert_fault();

    // Divergence: an overdraft transfer is not rejected at invoke time;
    // the balance check is deferred and the transaction fails at commit.
    let result = env.transact(|tx| {
        tx.call(&caller).run(move || {
            let client = ERC20Client::new(token);
            let balance = client.get_balance();
            client.transfer(&SPENDER, balance + 1);
        })
    });
    result.assert_commit_failed();
}
