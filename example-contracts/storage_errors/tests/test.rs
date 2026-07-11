use sdk::*;
use storage_errors::{StorageErrorClient, StorageErrorClientError};

/// The contract under test.
const CONTRACT_ADDR: ContractAddress = ContractAddress::new([0x01; 32]);
/// A separate "caller" address — client calls originate from here.
const CALLER_ADDR: ContractAddress = ContractAddress::new([0xFF; 32]);

/// Register the mock contract and pre-deploy at CONTRACT_ADDR.
fn setup() -> TestEnvironmentHandle {
    let env = sdk::TestEnvironment::no_cost();
    let contract = storage_errors::register_mock_contract(&env);
    contract.deploy_contract_to(&CONTRACT_ADDR);
    env
}

// -- Happy paths --

#[test]
fn vector_get_in_bounds() {
    let env = setup();
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
    let env = setup();
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
    let env = setup();
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
    let env = setup();
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
                Err(StorageErrorClientError::Library(
                    LibraryExn::StorageSerialization,
                )) => {}
                other => panic!(
                    "expected StorageSerialization abort, got {other:?}"
                ),
            }
        })
    });
    result.assert_success();
}

#[test]
fn missing_len_slot_still_reads_as_empty() {
    let env = setup();
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
    let env = setup();
    let result = env.transact(|tx| {
        tx.call(&CALLER_ADDR).run(|| {
            let client = StorageErrorClient::new(&CONTRACT_ADDR);
            client.initialize();

            let err = client.try_vector_get(5).unwrap_err();
            let lib = match err {
                StorageErrorClientError::Library(lib) => lib,
                other => panic!("Expected Library, got: {:?}", other),
            };
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
    let env = setup();
    let result = env.transact(|tx| {
        tx.call(&CALLER_ADDR).run(|| {
            let client = StorageErrorClient::new(&CONTRACT_ADDR);
            client.initialize();

            let err = client.try_vector_set(10, 999).unwrap_err();
            let lib = match err {
                StorageErrorClientError::Library(lib) => lib,
                other => panic!("Expected Library, got: {:?}", other),
            };
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
    let env = setup();
    let result = env.transact(|tx| {
        tx.call(&CALLER_ADDR).run(|| {
            let client = StorageErrorClient::new(&CONTRACT_ADDR);
            // Contract not initialized — vector len is 0, so index 0 is OOB.
            let err = client.try_vector_get(0).unwrap_err();
            match err {
                StorageErrorClientError::Library(
                    LibraryExn::StorageIndexOutOfBounds { len: 0, access: 0 },
                ) => {}
                other => panic!("Expected Library OOB, got: {:?}", other),
            }
        })
    });
    result.assert_success();
}

#[test]
fn map_get_missing_key_is_library_storage_error() {
    let env = setup();
    let result = env.transact(|tx| {
        tx.call(&CALLER_ADDR).run(|| {
            let client = StorageErrorClient::new(&CONTRACT_ADDR);
            client.initialize();

            let err = client.try_map_get(999).unwrap_err();
            let lib = match err {
                StorageErrorClientError::Library(lib) => lib,
                other => panic!("Expected Library, got: {:?}", other),
            };
            // get() on a missing key throws LibraryExn::Syscall with
            // kSyscallErrorCodeStorageKeyNotExist (key-not-exist is only
            // swallowed to None in try_get, not get).
            match lib {
                LibraryExn::Syscall(exn) => {
                    assert!(
                        exn.has_code(
                            sdk::sys::SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist
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
