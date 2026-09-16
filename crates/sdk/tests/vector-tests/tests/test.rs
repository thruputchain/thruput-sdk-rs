use std::cell::Cell;
use std::panic::AssertUnwindSafe;

use sdk::*;
use vector_tests::*;

static NONCE: SHA256Hash = sdk::sha256_static!("VECTOR_TEST_NONCE");

/// Helper: set up the fibonacci contract in a test environment.
fn setup() -> (TestEnvironmentHandle, SHA256Hash) {
    let env = TestEnvironment::no_cost();
    let contract = env.register_mock(VectorTestContract::__mock);
    let contract_hash = *contract.as_hash();
    (env, contract_hash)
}

#[test]
fn test_vector_basic_usage() {
    let (env, contract_hash) = setup();
    let addr = Cell::new(ContractAddress::default());

    let deploy_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });
    deploy_result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = VectorTestClient::new(&contract_addr);

                client.push_value(1);
                client.push_value(2);

                assert_eq!(client.get_value(0), 1);
                assert_eq!(client.get_value(1), 2);
            }))
    });
    result.assert_success();

    let result_expect_conflict = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = VectorTestClient::new(&contract_addr);

                client.push_value(1);
                client.push_value(2);

                assert_eq!(client.get_value(0), 1);
                assert_eq!(client.get_value(1), 2);
            }))
    });
    // vector push acquires exclusive lock on the vector size
    result_expect_conflict.assert_commit_failed_nondeterministically();

    env.advance_block();

    let read_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = VectorTestClient::new(&contract_addr);

                assert_eq!(client.get_value(0), 1);
                assert_eq!(client.get_value(1), 2);
            }))
    });
    read_result.assert_success();
}

#[test]
fn test_vector_pop() {
    let (env, contract_hash) = setup();
    let addr = Cell::new(ContractAddress::default());

    let deploy_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });
    deploy_result.assert_success();

    env.advance_block();

    // Push two values, pop one, verify
    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = VectorTestClient::new(&contract_addr);

                client.push_value(100);
                client.push_value(200);
                assert_eq!(client.values_len(), 2);

                client.pop_value();
                assert_eq!(client.values_len(), 1);
                assert_eq!(client.get_value(0), 100);
            }))
    });
    result.assert_success();
}

#[test]
fn test_iter_sum() {
    let (env, contract_hash) = setup();
    let addr = Cell::new(ContractAddress::default());

    let deploy_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });
    deploy_result.assert_success();
    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = VectorTestClient::new(&contract_addr);

                client.push_value(10);
                client.push_value(20);
                client.push_value(30);

                assert_eq!(client.sum_values(), 60);
            }))
    });
    result.assert_success();
}

#[test]
fn test_iter_mut_inc_all() {
    let (env, contract_hash) = setup();
    let addr = Cell::new(ContractAddress::default());

    let deploy_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });
    deploy_result.assert_success();
    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = VectorTestClient::new(&contract_addr);

                client.push_nnint();
                client.push_nnint();
                client.push_nnint();

                client.inc_nnint(0, 10);
                client.inc_nnint(1, 20);
                client.inc_nnint(2, 30);

                // Increment all by 5 via iter_mut
                client.inc_all_nnints(5);

                assert_eq!(client.get_nnint(0), 15);
                assert_eq!(client.get_nnint(1), 25);
                assert_eq!(client.get_nnint(2), 35);
            }))
    });
    result.assert_success();
}

#[test]
fn test_concurrent_shared_delete() {
    let (env, contract_hash) = setup();
    let addr = Cell::new(ContractAddress::default());

    let deploy_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });
    deploy_result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = VectorTestClient::new(&contract_addr);

                client.push_value(100);
                client.push_value(200);
                assert_eq!(client.values_len(), 2);
            }))
    });
    result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = VectorTestClient::new(&contract_addr);

                assert_eq!(client.values_len(), 2);
                client.delete_values();
            }))
    });
    result.assert_success();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = VectorTestClient::new(&contract_addr);

                assert_eq!(client.values_len(), 2);
                client.delete_values();
            }))
    });
    result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = VectorTestClient::new(&contract_addr);

                assert_eq!(client.values_len(), 0);
            }))
    });
    result.assert_success();
}

/// Deleting an element through its slot punches a hole that survives a
/// commit: `len` is unchanged across the block boundary, the index still
/// reads back as missing, and the surviving elements keep their indices.
#[test]
fn test_element_delete_is_a_durable_hole() {
    let (env, contract_hash) = setup();
    let addr = Cell::new(ContractAddress::default());

    let deploy_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });
    deploy_result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = VectorTestClient::new(&addr.get());
                client.push_value(10);
                client.push_value(20);
                client.push_value(30);
                assert_eq!(client.values_len(), 3);
            }))
    });
    result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = VectorTestClient::new(&addr.get());
                client.delete_value(1);

                // Within the same transaction: len is untouched, the hole
                // reads as absent, neighbours do not shift down.
                assert_eq!(client.values_len(), 3);
                assert_eq!(client.get_value_or_zero(0), 10);
                assert_eq!(client.get_value_or_zero(1), 0);
                assert_eq!(client.get_value_or_zero(2), 30);
            }))
    });
    result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = VectorTestClient::new(&addr.get());

                // After commit: still a 3-long vector with a hole at 1.
                assert_eq!(client.values_len(), 3);
                assert_eq!(client.get_value_or_zero(1), 0);

                // len is a high-water mark; the live count is what a caller
                // usually means by "how many are there".
                assert_eq!(client.count_live_values(), 2);
                assert_eq!(client.sum_values_sparse(), 40);
            }))
    });
    result.assert_success();
}

/// A transaction that deletes an element and then reverts leaves the element
/// in place: the delete is part of the reverted frame, not applied eagerly.
#[test]
fn test_element_delete_reverts_with_the_transaction() {
    let (env, contract_hash) = setup();
    let addr = Cell::new(ContractAddress::default());

    let deploy_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });
    deploy_result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = VectorTestClient::new(&addr.get());
                client.push_value(10);
                client.push_value(20);
            }))
    });
    result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            // The `-> ()` is load-bearing: the body diverges, and `run`
            // needs a return type that implements `WriteXDR`.
            .run(AssertUnwindSafe(|| -> () {
                let client = VectorTestClient::new(&addr.get());
                client.delete_value(0);
                assert_eq!(client.get_value_or_zero(0), 0);
                panic!("revert after deleting an element");
            }))
    });
    assert!(!result.is_success());

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = VectorTestClient::new(&addr.get());
                assert_eq!(client.values_len(), 2);
                assert_eq!(client.get_value_or_zero(0), 10);
                assert_eq!(client.get_value_or_zero(1), 20);
            }))
    });
    result.assert_success();
}

/// Unlike `push`/`pop`, deleting an element does not touch `len`, so two
/// transactions in the same block that delete *different* elements do not
/// conflict on the len slot.
#[test]
fn test_element_deletes_do_not_conflict_on_len() {
    let (env, contract_hash) = setup();
    let addr = Cell::new(ContractAddress::default());

    let deploy_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });
    deploy_result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = VectorTestClient::new(&addr.get());
                client.push_value(10);
                client.push_value(20);
                client.push_value(30);
            }))
    });
    result.assert_success();

    env.advance_block();

    // Two same-block transactions deleting disjoint elements both commit.
    let first = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                VectorTestClient::new(&addr.get()).delete_value(0);
            }))
    });
    first.assert_success();

    let second = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                VectorTestClient::new(&addr.get()).delete_value(2);
            }))
    });
    second.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = VectorTestClient::new(&addr.get());
                assert_eq!(client.values_len(), 3);
                assert_eq!(client.count_live_values(), 1);
                assert_eq!(client.sum_values_sparse(), 20);
            }))
    });
    result.assert_success();
}

/// Deleting an NNInt element is not durable in the way a caller might
/// expect: a later `inc` treats the missing slot as zero and recreates it.
#[test]
fn test_deleted_nnint_element_resurrects_on_inc() {
    let (env, contract_hash) = setup();
    let addr = Cell::new(ContractAddress::default());

    let deploy_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });
    deploy_result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = VectorTestClient::new(&addr.get());
                client.push_nnint();
                client.push_nnint();
                client.inc_nnint(0, 100);
                client.inc_nnint(1, 200);
            }))
    });
    result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = VectorTestClient::new(&addr.get());
                client.delete_nnint(0);
                // The accumulated total is gone, so the increment starts
                // from zero rather than resuming at 100.
                client.inc_nnint(0, 5);
                assert_eq!(client.get_nnint(0), 5);
                assert_eq!(client.get_nnint(1), 200);
            }))
    });
    result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = VectorTestClient::new(&addr.get());
                assert_eq!(client.get_nnint(0), 5);
                assert_eq!(client.get_nnint(1), 200);
            }))
    });
    result.assert_success();
}

/// Deleting a map entry that holds a whole `Vector` clears the nested
/// vector's elements and its len, leaving sibling entries alone.
#[test]
fn test_nested_vector_delete_through_map_entry() {
    let (env, contract_hash) = setup();
    let addr = Cell::new(ContractAddress::default());

    let deploy_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });
    deploy_result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = VectorTestClient::new(&addr.get());
                client.nested_push(1, 10);
                client.nested_push(1, 20);
                client.nested_push(2, 30);
                assert_eq!(client.nested_len(1), 2);
                assert_eq!(client.nested_len(2), 1);
            }))
    });
    result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = VectorTestClient::new(&addr.get());
                client.nested_delete(1);
                assert_eq!(client.nested_len(1), 0);
                assert_eq!(client.nested_len(2), 1);
            }))
    });
    result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let client = VectorTestClient::new(&addr.get());
                assert_eq!(client.nested_len(1), 0);
                assert_eq!(client.nested_len(2), 1);
                assert_eq!(client.nested_get(2, 0), 30);
            }))
    });
    result.assert_success();
}
