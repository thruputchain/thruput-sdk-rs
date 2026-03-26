#![allow(unused)]

use exceptions::*;
use sdk::sys::{SyscallErrorCode, SyscallNo};
use sdk::*;

pub static TEST_NONCE: sdk::SHA256Hash =
    sdk::sha256_static!("EXCEPTIONS_TEST_NONCE");

pub fn exceptions_test(contract_hash: &SHA256Hash) {
    // Deploy the contract under test
    let addr = deploy_contract(&TEST_NONCE, contract_hash);

    // Create a client to interact with the contract
    let mut client = Client::new(&addr);

    client.hello();

    assert_eq!(client.try_hello(), Ok(()));

    let n = client.add(6);
    assert_eq!(n, 7);

    assert_eq!(client.try_add(6), Ok(7));

    match client.try_aborts_with_my_err() {
        Err(ClientError::MyErr(ref my_err)) => {
            println!("OK: {:?}", my_err);
        }
        Err(ce) => {
            panic!("Unexpected call error: {:?}", ce)
        }
        r => panic!("Unexpected result: {:?}", r),
    }

    // Throw something undeclared with `throws`.
    // With selector-based dispatch, `abort(&Thing)` serializes with Thing's
    // selector. Since Thing is not declared in `throws`, the caller's decoder
    // puts it in the Unknown variant.
    match client.try_throw_thing() {
        Err(ClientError::Unknown(ref raw)) => {
            println!("OK: unknown error with selector 0x{:016X}", raw.selector())
        }
        Err(ClientError::MyErr(ref my_err)) => {
            // If Thing and MyErr happen to have the same selector (unlikely),
            // we'd get here. Accept either outcome.
            println!("OK: decoded as MyErr: {:?}", my_err)
        }
        r => panic!("Unexpected result: {:?}", r),
    }

    match client.try_syscall_err() {
        Err(ClientError::Library(ref lib_exn)) => {
            match lib_exn {
                LibraryExn::Syscall(sys_exn) => {
                    if sys_exn.has_code(SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist)
                        && sys_exn.callno() == SyscallNo::kBytestringGetLen {
                        println!("OK: {:?}", sys_exn);
                    } else {
                        panic!("Unexpected syscall exception: {:?}", sys_exn);
                    }
                }
                _ => panic!("Unexpected library exn: {:?}", lib_exn),
            }
        }
        r => panic!("Unexpected result: {:?}", r),
    }
}

#[allow(unused)]
pub fn panic_test(contract_hash: &SHA256Hash) {
    //
    let addr = deploy_contract(&TEST_NONCE, contract_hash);
    //
    let mut client = Client::new(&addr);

    match client.try_panics() {
        Err(ClientError::Fault(f)) => {
            println!("OK: Got fault: {}", f);
        }
        Err(e) => {
            panic!("Unexpected error: {:?}", e);
        }
        Ok(()) => {
            panic!("Unexpected success")
        }
    }
}
