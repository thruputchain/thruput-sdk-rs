use sdk::*;
use storage_errors::{StorageErrorClient, StorageErrorClientError};

/// The contract under test.
const CONTRACT_ADDR: ContractAddress = ContractAddress::new([0x01; 32]);
/// A separate "caller" address — client calls originate from here.
const CALLER_ADDR: ContractAddress = ContractAddress::new([0xFF; 32]);

/// Register the mock contract and pre-deploy at CONTRACT_ADDR.
fn setup() -> TestEnvironmentHandle {
    let env = sdk::TestEnvironment::no_cost();
    let contract =
        env.register_mock(storage_errors::StorageErrorContract::__mock);
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

// -- Error paths: storage errors must propagate as LibraryExn::Storage --

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
                LibraryExn::Storage(
                    sdk::storage::StorageError::IndexOutOfBounds {
                        len: 3,
                        access: 5,
                    },
                ) => {}
                other => panic!(
                    "Expected Storage(IndexOutOfBounds {{ len: 3, access: 5 }}), got: {:?}",
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
                LibraryExn::Storage(
                    sdk::storage::StorageError::IndexOutOfBounds {
                        len: 3,
                        access: 10,
                    },
                ) => {}
                other => panic!(
                    "Expected Storage(IndexOutOfBounds {{ len: 3, access: 10 }}), got: {:?}",
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
            // Contract not initialized — generated export panics
            // when storage isn't present, producing a Fault.
            let err = client.try_vector_get(0).unwrap_err();
            match err {
                StorageErrorClientError::Library(LibraryExn::Storage(e)) => {
                    assert!(matches!(
                        e,
                        sdk::storage::StorageError::IndexOutOfBounds {
                            len: 0,
                            access: 0
                        }
                    ));
                }
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
            match lib {
                LibraryExn::Storage(e) => {
                    assert!(
                        matches!(e, sdk::storage::StorageError::Sys { exn }
                            if exn.has_code(sdk::sys::SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist)),
                        "Expected StorageError::Sys(StorageKeyNotExist), got: {:?}",
                        e
                    );
                }
                other => panic!(
                    "Expected LibraryExn::Storage, got: {:?}",
                    other
                ),
            }
        })
    });
    result.assert_success();
}
