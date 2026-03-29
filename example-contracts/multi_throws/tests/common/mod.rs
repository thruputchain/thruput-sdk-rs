#![allow(unused)]

use multi_throws::*;
use sdk::*;

pub static TEST_NONCE: sdk::SHA256Hash =
    sdk::sha256_static!("MULTI_THROWS_TEST_NONCE");

pub static TEST_NONCE_2: sdk::SHA256Hash =
    sdk::sha256_static!("MULTI_THROWS_TEST_NONCE_2");

pub fn multi_throws_test(contract_hash: &SHA256Hash) {
    let addr = deploy_contract(&TEST_NONCE, contract_hash);
    let client = MultiClient::new(&addr);

    // Test 1: Success path
    assert_eq!(client.try_succeed(), Ok(42));
    assert_eq!(client.succeed(), 42);

    // Test 2: AlphaError::NotFound dispatch
    match client.try_throw_alpha_not_found() {
        Err(MultiClientError::AlphaError(AlphaError::NotFound)) => {
            println!("OK: got AlphaError::NotFound");
        }
        other => panic!("expected AlphaError::NotFound, got {:?}", other),
    }

    // Test 3: AlphaError::Overflow dispatch
    match client.try_throw_alpha_overflow() {
        Err(MultiClientError::AlphaError(AlphaError::Overflow)) => {
            println!("OK: got AlphaError::Overflow");
        }
        other => panic!("expected AlphaError::Overflow, got {:?}", other),
    }

    // Test 4: BetaError::Denied dispatch
    match client.try_throw_beta_denied() {
        Err(MultiClientError::BetaError(BetaError::Denied)) => {
            println!("OK: got BetaError::Denied");
        }
        other => panic!("expected BetaError::Denied, got {:?}", other),
    }

    // Test 5: Undeclared error goes to Unknown, try_as can decode it
    match client.try_throw_undeclared() {
        Err(MultiClientError::Unknown(ref raw)) => {
            println!(
                "OK: unknown error with selector 0x{:016X}",
                raw.selector()
            );
            // The caller knows about SecretError and can try to decode it
            let secret = raw.try_as::<SecretError>();
            assert_eq!(secret, Some(SecretError));
            println!("OK: try_as decoded SecretError successfully");
        }
        other => panic!("expected Unknown, got {:?}", other),
    }

    // Test 6: Error forwarding through a call chain.
    // Deploy a second instance; call forward_alpha on the first, which
    // calls throw_alpha_not_found on the second via the non-try method.
    // re_throw should forward the error so we see AlphaError here.
    let addr2 = deploy_contract(&TEST_NONCE_2, contract_hash);
    match client.try_forward_alpha(addr2.clone()) {
        Err(MultiClientError::AlphaError(AlphaError::NotFound)) => {
            println!("OK: forwarded AlphaError::NotFound through call chain");
        }
        other => {
            panic!("expected forwarded AlphaError::NotFound, got {:?}", other)
        }
    }

    // Test 7: Forwarding an undeclared error through a call chain.
    // The inner contract throws SecretError, the middle contract re-throws,
    // and we see it as Unknown with the original selector.
    match client.try_forward_undeclared(addr2.clone()) {
        Err(MultiClientError::Unknown(ref raw)) => {
            let secret = raw.try_as::<SecretError>();
            assert_eq!(secret, Some(SecretError));
            println!(
                "OK: forwarded SecretError through call chain as Unknown"
            );
        }
        other => {
            panic!("expected forwarded Unknown(SecretError), got {:?}", other)
        }
    }

    // Test 8: Forwarding a fault through a call chain.
    // The inner contract panics, the middle contract's non-try method
    // propagates it as a panic (fault), and we see Fault here.
    match client.try_forward_fault(addr2) {
        Err(MultiClientError::Fault(ref f)) => {
            println!("OK: forwarded fault through call chain: {}", f);
        }
        other => panic!("expected forwarded Fault, got {:?}", other),
    }
}
