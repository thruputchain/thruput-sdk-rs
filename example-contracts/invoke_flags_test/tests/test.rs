use invoke_flags_test::*;
use sdk::*;

const DEPLOYED_ADDR: ContractAddress = ContractAddress::new([0x42; 32]);

fn setup() -> TestEnvironmentHandle {
    let env = TestEnvironment::no_cost();
    let contract = invoke_flags_test::register_mock_contract(&env);
    env.setup_create_deployment(&DEPLOYED_ADDR, &contract.as_contract());
    env
}

#[test]
fn concurrent_write_conflict_at_commit() {
    let env = setup();

    let result1 = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller_a"))
            .run(|| {
                KeyValueStoreClient::new(&DEPLOYED_ADDR).write(42);
            })
    });
    result1.assert_success();

    // Concurrent tx in the same block writes a different value to the same key.
    let result2 = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller_b"))
            .run(|| {
                KeyValueStoreClient::new(&DEPLOYED_ADDR).write(99);
            })
    });
    // Without TryInvoke, the conflict is caught at commit time.
    result2.assert_commit_failed_nondeterministically();
}

#[test]
fn try_invoke_conflict_at_frame_close() {
    let env = setup();

    let result1 = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller_a"))
            .run(|| {
                KeyValueStoreClient::new(&DEPLOYED_ADDR).write(42);
            })
    });
    result1.assert_success();

    // Concurrent tx writes a different value via a TryInvoke child call.
    // The conflict is detected when the child frame closes, not at commit.
    let result2 = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller_b"))
            .run(|| {
                let err = KeyValueStoreClient::new(&DEPLOYED_ADDR)
                    .with_commit_locking()
                    .try_write(99);
                assert!(
                    err.is_err(),
                    "TryInvoke should detect conflict at frame close"
                );
            })
    });
    // The root frame handled the error, so the tx commits OK.
    result2.assert_success();
}

#[test]
fn read_only_rejects_write() {
    let env = setup();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(|| {
                let err = KeyValueStoreClient::new(&DEPLOYED_ADDR)
                    .with_read_only()
                    .try_write(42);
                assert!(err.is_err(), "ReadOnly should reject storage writes");
            })
    });
    result.assert_success();
}

#[test]
fn no_flags_success() {
    let env = setup();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(|| {
                let result = KeyValueStoreClient::new(&DEPLOYED_ADDR).try_write(42);
                assert!(result.is_ok(), "No flags should succeed");
            })
    });
    result.assert_success();
}
