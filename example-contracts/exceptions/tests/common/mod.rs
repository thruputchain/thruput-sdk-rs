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
    let client = Client::new(&addr);

    client.hello();

    assert_eq!(client.try_hello(), Ok(()));

    let n = client.add(6);
    assert_eq!(n, 7);

    assert_eq!(client.try_add(6), Ok(7));

    match client.try_throws_my_exn() {
        Err(e) => match_exn!(e, {
            MyExn => println!("OK: MyExn"),
            _ => panic!("Unexpected call error: {e:?}"),
        }),
        r => panic!("Unexpected result: {:?}", r),
    }

    // Throw something undeclared with `throws`. `throw(Thing)` serializes
    // with Thing's own selector; `Thing` is private to the contract, so no
    // caller can name it and no typed arm can match it — it reaches `_`.
    match client.try_throw_thing() {
        Err(ref e @ CallError::Exception(ref raw)) => {
            assert_eq!(
                match_exn!(e, {
                    MyExn => "my-exn",
                    BigExn { .. } => "big-exn",
                    _ => "unmatched",
                }),
                "unmatched"
            );
            println!(
                "OK: undeclared error with selector 0x{:016X}",
                raw.selector()
            )
        }
        r => panic!("Unexpected result: {:?}", r),
    }

    // `LibraryExn` is matched like any other exception type, so its nested
    // variant is reachable in a single arm rather than through a wrapper.
    match client.try_syscall_err() {
        Err(e) => match_exn!(e, {
            LibraryExn::Syscall(sys_exn) => {
                if sys_exn.has_code(
                    SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist,
                ) && sys_exn.callno() == SyscallNo::kBytestringGetLen
                {
                    println!("OK: {:?}", sys_exn);
                } else {
                    panic!("Unexpected syscall exception: {:?}", sys_exn);
                }
            }
            _ => panic!("Unexpected error: {e:?}"),
        }),
        r => panic!("Unexpected result: {:?}", r),
    }

    // A 600-byte exception payload exceeds the SDK's legacy 500-byte encode
    // buffer but fits the protocol's 1024-byte return cap — it must arrive
    // intact at the caller, not die at encode time.
    match client.try_throw_big_exn() {
        Err(e) => match_exn!(e, {
            BigExn { payload } => {
                assert_eq!(payload, [0xAB; 600]);
                println!("OK: 600-byte exception propagated intact");
            }
            _ => panic!("Unexpected error: {e:?}"),
        }),
        r => panic!("Unexpected result: {:?}", r),
    }
}

#[allow(unused)]
pub fn panic_test(contract_hash: &SHA256Hash) {
    //
    let addr = deploy_contract(&TEST_NONCE, contract_hash);
    //
    let client = Client::new(&addr);

    match client.try_panics() {
        Err(CallError::Fault(f)) => {
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
