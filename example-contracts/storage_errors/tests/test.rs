use storage_errors::StorageErrorClient;
use thruput_sdk::*;

/// The contract under test.
const CONTRACT_ADDR: ContractAddress = ContractAddress::new([0x01; 32]);
/// A separate "caller" address — client calls originate from here.
const CALLER_ADDR: ContractAddress = ContractAddress::new([0xFF; 32]);

static NONCE: SHA256Hash = thruput_sdk::sha256_static!("STORAGE_ERRORS_NONCE");
static NONCE_A: SHA256Hash =
    thruput_sdk::sha256_static!("STORAGE_ERRORS_NONCE_A");
static NONCE_B: SHA256Hash =
    thruput_sdk::sha256_static!("STORAGE_ERRORS_NONCE_B");

/// Register the mock contract and pre-deploy at CONTRACT_ADDR.
fn setup() -> (TestEnvironmentHandle, SHA256Hash) {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let contract = storage_errors::register_mock_contract(&env);
    let contract_hash = contract.as_hash().clone();
    contract.deploy_contract_to(&CONTRACT_ADDR);
    (env, contract_hash)
}

// -- Happy paths --

#[test]
fn vector_get_in_bounds() {
    let (env, _) = setup();
    let result = env.transact(|tx| {
        tx.call(&CALLER_ADDR).run(|| {
            let client = StorageErrorClient::new(&CONTRACT_ADDR);
            client.initialize();
            assert_eq!(client.vector_get(0), 10);
            assert_eq!(client.vector_get(1), 20);
            assert_eq!(client.vector_get(2), 30);
            assert_eq!(client.vector_len(), 3);
        })
    });
    result.assert_success();
}

#[test]
fn map_set_and_get() {
    let (env, _) = setup();
    let result = env.transact(|tx| {
        tx.call(&CALLER_ADDR).run(|| {
            let client = StorageErrorClient::new(&CONTRACT_ADDR);
            client.initialize();
            client.map_set(42, 100);
            assert_eq!(client.map_get(42), 100);
        })
    });
    result.assert_success();
}

#[test]
fn map_try_get_missing_returns_zero() {
    let (env, _) = setup();
    let result = env.transact(|tx| {
        tx.call(&CALLER_ADDR).run(|| {
            let client = StorageErrorClient::new(&CONTRACT_ADDR);
            client.initialize();
            assert_eq!(client.map_try_get(999), 0);
        })
    });
    result.assert_success();
}

// -- Vector len slot corruption --

#[test]
fn corrupted_len_slot_aborts_instead_of_reading_zero() {
    let (env, _) = setup();
    let result = env.transact(|tx| {
        tx.call(&CALLER_ADDR).run(|| {
            let client = StorageErrorClient::new(&CONTRACT_ADDR);
            client.initialize();
            assert_eq!(client.vector_len(), 3);

            client.corrupt_len_slot();

            // A len slot that exists but does not decode as u32 must abort
            // typed — NOT read as "empty vector" (which would let the next
            // push clobber whatever lives at element 0).
            match client.try_vector_len() {
                Err(e) => match_exn!(e, {
                    LibraryExn::StorageSerialization => {}
                    _ => panic!(
                        "expected StorageSerialization abort, got {e:?}"
                    ),
                }),
                Ok(len) => panic!(
                    "expected StorageSerialization abort, got Ok({len})"
                ),
            }
        })
    });
    result.assert_success();
}

#[test]
fn missing_len_slot_still_reads_as_empty() {
    let (env, _) = setup();
    let result = env.transact(|tx| {
        tx.call(&CALLER_ADDR).run(|| {
            let client = StorageErrorClient::new(&CONTRACT_ADDR);
            // No initialize: len slot never written -> empty vector.
            assert_eq!(client.vector_len(), 0);
        })
    });
    result.assert_success();
}

// -- Error paths: storage errors must propagate as flat LibraryExn variants --

#[test]
fn vector_get_out_of_bounds_is_library_storage_error() {
    let (env, _) = setup();
    let result = env.transact(|tx| {
        tx.call(&CALLER_ADDR).run(|| {
            let client = StorageErrorClient::new(&CONTRACT_ADDR);
            client.initialize();

            let err = client.try_vector_get(5).unwrap_err();
            let lib = err
                .try_as::<LibraryExn>()
                .unwrap_or_else(|| panic!("Expected LibraryExn, got: {err:?}"));
            match lib {
                LibraryExn::StorageIndexOutOfBounds { len: 3, access: 5 } => {}
                other => panic!(
                    "Expected StorageIndexOutOfBounds {{ len: 3, access: 5 }}, got: {:?}",
                    other
                ),
            }
        })
    });
    result.assert_success();
}

#[test]
fn vector_set_out_of_bounds_is_library_storage_error() {
    let (env, _) = setup();
    let result = env.transact(|tx| {
        tx.call(&CALLER_ADDR).run(|| {
            let client = StorageErrorClient::new(&CONTRACT_ADDR);
            client.initialize();

            let err = client.try_vector_set(10, 999).unwrap_err();
            let lib = err
                .try_as::<LibraryExn>()
                .unwrap_or_else(|| panic!("Expected LibraryExn, got: {err:?}"));
            match lib {
                LibraryExn::StorageIndexOutOfBounds { len: 3, access: 10 } => {}
                other => panic!(
                    "Expected StorageIndexOutOfBounds {{ len: 3, access: 10 }}, got: {:?}",
                    other
                ),
            }
        })
    });
    result.assert_success();
}

#[test]
fn vector_get_on_uninitialized_contract_is_fault() {
    let (env, _) = setup();
    let result = env.transact(|tx| {
        tx.call(&CALLER_ADDR).run(|| {
            let client = StorageErrorClient::new(&CONTRACT_ADDR);
            // Contract not initialized — vector len is 0, so index 0 is OOB.
            let err = client.try_vector_get(0).unwrap_err();
            match_exn!(err, {
                LibraryExn::StorageIndexOutOfBounds { len: 0, access: 0 } => {}
                _ => panic!("Expected LibraryExn OOB, got: {err:?}"),
            })
        })
    });
    result.assert_success();
}

#[test]
fn map_get_missing_key_is_library_storage_error() {
    let (env, _) = setup();
    let result = env.transact(|tx| {
        tx.call(&CALLER_ADDR).run(|| {
            let client = StorageErrorClient::new(&CONTRACT_ADDR);
            client.initialize();

            let err = client.try_map_get(999).unwrap_err();
            let lib = err
                .try_as::<LibraryExn>()
                .unwrap_or_else(|| panic!("Expected LibraryExn, got: {err:?}"));
            // get() on a missing key throws LibraryExn::Syscall with
            // kSyscallErrorCodeStorageKeyNotExist (key-not-exist is only
            // swallowed to None in try_get, not get).
            match lib {
                LibraryExn::Syscall(exn) => {
                    assert!(
                        exn.has_code(
                            thruput_sdk::sys::SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist
                        ),
                        "Expected StorageKeyNotExist error code, got: {:?}",
                        exn
                    );
                }
                other => panic!(
                    "Expected LibraryExn::Syscall(StorageKeyNotExist), got: {:?}",
                    other
                ),
            }
        })
    });
    result.assert_success();
}

// -- Live-guard checks --

#[test]
fn guard_dropped_before_early_return_succeeds() {
    let (env, _) = setup();
    let result = env.transact(|tx| {
        tx.call(&CALLER_ADDR).run(|| {
            let client = StorageErrorClient::new(&CONTRACT_ADDR);
            client.initialize();
            // Guard is dropped (and flushed) before the early return —
            // the live-guard check must not fire, and the write must land.
            client.early_return_after_drop();
            assert_eq!(client.get_plain(), 7);
        })
    });
    result.assert_success();
}

#[test]
fn early_return_with_live_guard_fails_loudly() {
    let caught =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (env, contract_hash) = setup();
            env.transact(|tx| {
                tx.call(&ContractAddress::named("test")).run(move || -> () {
                    let addr = deploy_contract(&NONCE, &contract_hash);
                    let client = StorageErrorClient::new(&addr);
                    client.initialize();
                    // Deliberately `try_`: the check must be unswallowable. A
                    // plain panic would be rewritten to a Deterministic error
                    // and handed back here as an ordinary `Err`, which this
                    // call would quietly absorb. Reaching the line below at
                    // all means the check failed to fire hard enough.
                    let res = client.try_early_return_with_live_guard();
                    panic!("swallowed as a call result: {res:?}");
                })
            })
            .assert_success();
        }));

    let payload = caught.expect_err(
        "early return with a live ValueMut must fail, not silently succeed",
    );
    // The live-guard check unwinds as Unrecoverable, so it blows past the
    // `try_` call above rather than being caught by it. (The human-readable
    // diagnostic naming the guard goes to stderr, not the payload.)
    let err = payload.downcast_ref::<InvokeError>().unwrap_or_else(|| {
        let msg = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "<opaque payload>".to_string());
        panic!("expected an Unrecoverable InvokeError, got: {msg}");
    });
    assert_eq!(
        err.error_type,
        InvokeErrorCode::Unrecoverable,
        "a dropped storage write is a contract bug, not a catchable abort"
    );
}

#[test]
fn guard_across_nested_call_is_not_a_false_positive() {
    let (env, contract_hash) = setup();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let a = deploy_contract(&NONCE_A, &contract_hash);
            let b = deploy_contract(&NONCE_B, &contract_hash);
            let ca = StorageErrorClient::new(&a);
            let cb = StorageErrorClient::new(&b);
            ca.initialize();
            cb.initialize(); // b's vector has len 3

            // a holds a ValueMut across the call into b; b's normal return
            // must not trip the live-guard check (b's frame has no guards).
            assert_eq!(ca.guard_across_call(b.clone()), 4); // 0 + 1 + 3

            // a's guard flushed on drop.
            assert_eq!(ca.get_plain(), 4);
        })
    })
    .assert_success();
}
