#![allow(unused)]

use sdk::*;
use exceptions::*;
use sdk::sys::{SyscallNo, SyscallErrorCode};
use sdk::call::CallError;

pub static TEST_NONCE: sdk::SHA256Hash = sdk::sha256_static!("EXCEPTIONS_TEST_NONCE");

pub fn exceptions_test(
    contract_hash: &SHA256Hash,
) {
    // Deploy the contract under test
    let addr = deploy_contract(&TEST_NONCE, contract_hash);

    // Create a client to interact with the contract
    let mut client = ContractClient::new(&addr);

    client.hello();

    assert_eq!(client.try_hello(), Ok(()));

    let n = client.add(6); assert_eq!(n, 7);

    assert_eq!(client.try_add(6), Ok(7));

    match client.try_aborts_with_my_err() {
        Err(CallError::Abort(ref cross_exn)) => {
            if let Some(my_err) = cross_exn.as_contract_exn() {
                let _: &MyErr = my_err;

                println!("OK: {:?}", cross_exn);
            } else {
                panic!("Unexpected abort exception: {:?}", cross_exn)
            }
        }
        Err(ce) => {
            panic!("Unexpected call error: {:?}", ce)
        }
        r => panic!("Unexpected result: {:?}", r),
    }

    // Throw something undeclared with `throws`
    match client.try_throw_thing() {
        Err(CallError::Abort(ref cross_exn)) if cross_exn.is_unknown() =>
            println!("OK: {:?}", cross_exn),
        r => panic!("Unexpected result: {:?}", r),
    }

    match client.try_syscall_err() {
        Err(ref e @ CallError::Abort(ref cross_exn)) => {
            if let Some(exn) = cross_exn.as_library_exn() {
                match exn {
                    LibraryExn::Syscall(sys_exn) => {
                        if sys_exn.has_code(SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist)
                            && sys_exn.callno() == SyscallNo::kBytestringGetLen {
                            println!("OK: {:?}", cross_exn);
                        } else {
                            panic!("Unexpected syscall exception: {}", sys_exn);
                        }
                    }
                    _ => panic!("Unexpected library exn: {}", e),
                }
            } else {
                panic!("Unexpected abort: {:?}", cross_exn);
            }
        },
        r => panic!("Unexpected result: {:?}", r),
    }
}



#[allow(unused)]
pub fn panic_test(contract_hash: &SHA256Hash) {
    //
    let addr = deploy_contract(&TEST_NONCE, contract_hash);
    //
    let mut client = ContractClient::new(&addr);

    match client.try_panics() {
        Err(CallError::Fault(f)) => {
            println!("OK: Got fault: {}", f);
        }
        Err(e) => {
            panic!("Unexpected error: {}", e);
        }
        Ok(()) => {
            panic!("Unexpected success")
        }
    }
}
