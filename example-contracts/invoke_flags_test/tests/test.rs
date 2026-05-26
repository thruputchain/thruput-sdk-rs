use std::cell::Cell;
use std::panic::AssertUnwindSafe;

use invoke_flags_test::*;
use sdk::*;

static NONCE: SHA256Hash = sdk::sha256_static!("INVOKE_FLAGS_TEST_NONCE");

fn setup() -> (TestEnvironmentHandle, SHA256Hash) {
    let env = TestEnvironment::no_cost();
    let contract = invoke_flags_test::register_mock_contract(&env);
    let contract_hash = contract.as_hash().clone();
    (env, contract_hash)
}

fn deploy(
    env: &TestEnvironmentHandle,
    contract_hash: &SHA256Hash,
) -> ContractAddress {
    let addr = Cell::new(ContractAddress::default());
    let hash = *contract_hash;
    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("deployer"))
            .run(AssertUnwindSafe(|| {
                addr.set(deploy_contract(&NONCE, &hash));
            }))
    });
    result.assert_success();
    env.advance_block();
    addr.get()
}

#[test]
fn concurrent_write_conflict_at_commit() {
    let (env, contract_hash) = setup();
    let deployed_addr = deploy(&env, &contract_hash);

    let result1 = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller_a"))
            .run(AssertUnwindSafe(|| {
                KeyValueStoreClient::new(&deployed_addr).write(42);
            }))
    });
    result1.assert_success();

    // Concurrent tx in the same block writes a different value to the same key.
    let result2 = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller_b"))
            .run(AssertUnwindSafe(|| {
                KeyValueStoreClient::new(&deployed_addr).write(99);
            }))
    });
    // Without TryInvoke, the conflict is caught at commit time.
    result2.assert_commit_failed_nondeterministically();
}

#[test]
fn try_invoke_conflict_at_frame_close() {
    let (env, contract_hash) = setup();
    let deployed_addr = deploy(&env, &contract_hash);

    let result1 = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller_a"))
            .run(AssertUnwindSafe(|| {
                KeyValueStoreClient::new(&deployed_addr).write(42);
            }))
    });
    result1.assert_success();

    // Concurrent tx writes a different value via a TryInvoke child call.
    // The conflict is detected when the child frame closes, not at commit.
    let result2 = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller_b"))
            .run(AssertUnwindSafe(|| {
                let client = KeyValueStoreClient::new(&deployed_addr);
                let err = client.with_commit_locking().try_write(99);
                assert!(
                    err.is_err(),
                    "TryInvoke should detect conflict at frame close"
                );
            }))
    });
    // The root frame handled the error, so the tx commits OK.
    result2.assert_success();
}

#[test]
fn read_only_rejects_write() {
    let (env, contract_hash) = setup();
    let deployed_addr = deploy(&env, &contract_hash);

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = KeyValueStoreClient::new(&deployed_addr);
                let err = client.with_read_only().try_write(42);
                assert!(
                    err.is_err(),
                    "ReadOnly should reject storage writes"
                );
            }))
    });
    result.assert_success();
}

#[test]
fn no_flags_success() {
    let (env, contract_hash) = setup();
    let deployed_addr = deploy(&env, &contract_hash);

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = KeyValueStoreClient::new(&deployed_addr);
                let result = client.try_write(42);
                assert!(
                    result.is_ok(),
                    "No flags should succeed"
                );
            }))
    });
    result.assert_success();
}

