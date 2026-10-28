//! Runs the [`thruput_erc20_test`] conformance suite in the native test
//! environment. [`test_erc20_contract`] checks a deployed contract's results
//! and emitted events. [`test_native_token_precompile`] holds the node's
//! native-token precompile to the same expectations, apart from its
//! documented divergences.

pub use thruput_erc20_test::*;

use thruput_sdk::hash_ident;
use thruput_sdk::{
    ContractAddress, MatchExn, Named, SHA256Hash, TestEnvironmentHandle,
};

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
    let expected: Vec<thruput_erc20_if::StdERC20Event> =
        result.assert_return_as();
    check_events(&expected, result.events());
}

// Print out a transaction's event log
fn print_events(result: &thruput_sdk::TransactResult) {
    use thruput_erc20_if::*;
    use thruput_sdk::EmittedEvent;

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
/// - `initialize` always faults: the genesis root plays the reference's
///   stored contract owner, so there is nothing to initialize. `mint`
///   follows the reference's owner check, with the genesis root as
///   the owner: any other caller gets the reference's
///   [`thruput_erc20_if::ERC20Error::MustBeContractOwner`] rejection.
/// - `total_supply` tracks mints; nothing is pre-minted in the
///   test environment, so the supply is exactly what this test mints.
///
/// An overdraft `transfer` used to be a third divergence: the balance check
/// was deferred, so it passed invoke and failed at commit. As of test-ffi
/// 0.21.0 the precompile throws
/// [`thruput_erc20_if::ERC20Error::TooLarge`] at invoke time like the
/// reference contract, so it is asserted the same way the rest of this file
/// asserts a typed throw: `try_*` plus a match on the error.
///
/// Those divergences are asserted here too, so a change on either side
/// shows up as a regression.
pub fn test_native_token_precompile(env: TestEnvironmentHandle) {
    use thruput_erc20_if::*;
    use thruput_xdr::genesis_addresses::{
        ADDRESS_GENESIS_ROOT, ADDRESS_NATIVE_TOKEN,
    };

    let caller = ContractAddress::named("erc20_test");
    let token = &ADDRESS_NATIVE_TOKEN;

    const BALANCE: Amount = 101;
    // Nothing is pre-minted in the test environment, so after the funding
    // mint below the total supply is exactly the caller's balance.
    const TOTAL_SUPPLY: Amount = BALANCE;

    // Fund the test caller by minting as the genesis root, the only caller
    // the native token accepts for `mint`.
    let result = env.transact(|tx| {
        tx.call(&ADDRESS_GENESIS_ROOT).run(move || {
            ERC20Client::new(token).mint(&caller, BALANCE);
        })
    });
    result.assert_success();
    env.advance_block();

    // Run the standard ERC20 test subset that applies to a pre-funded token
    // without mint, and check the emitted events against the model.
    let result = env.transact(|tx| {
        tx.call(&caller).run(move || {
            let model = erc20_funded_test(token, BALANCE, TOTAL_SUPPLY);
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

            assert_eq!(
                client
                    .try_transfer_from(&caller, &SPENDER, 6)
                    .unwrap_err()
                    .try_as(),
                Some(ERC20Error::TooLarge),
            );

            client.transfer_from(&caller, &SPENDER, 5);
            assert_eq!(client.allowance(&caller, &SPENDER), 0);
            assert_eq!(client.get_balance_of(&caller), owner_balance - 5);
            assert_eq!(client.get_balance(), spender_balance + 5);
        })
    });
    result.assert_success();
    env.advance_block();

    // Divergence: initialize has no meaning for the native token and always
    // faults rather than throwing a typed ERC20Error.
    let result = env.transact(|tx| {
        tx.call(&caller).run(move || {
            ERC20Client::new(token).initialize(&caller);
        })
    });
    result.assert_fault();

    // mint follows the reference's owner check with the genesis root as the
    // owner: any other caller gets the typed rejection.
    let result = env.transact(|tx| {
        tx.call(&caller).run(move || {
            let client = ERC20Client::new(token);
            assert_eq!(
                client.try_mint(&caller, 1).unwrap_err().try_as(),
                Some(ERC20Error::MustBeContractOwner),
            );
        })
    });
    result.assert_success();

    // Overdrafts are rejected at invoke time with the same error the
    // reference contract throws, asserted the same way erc20-test does
    // (`try_*` + `try_as`) so the two are directly comparable.
    let result = env.transact(|tx| {
        tx.call(&caller).run(move || {
            let client = ERC20Client::new(token);
            let balance = client.get_balance();
            assert_eq!(
                client
                    .try_transfer(&SPENDER, balance + 1)
                    .unwrap_err()
                    .try_as(),
                Some(ERC20Error::TooLarge),
            );
        })
    });
    result.assert_success();
}
