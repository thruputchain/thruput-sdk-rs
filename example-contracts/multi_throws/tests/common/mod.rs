#![allow(unused)]

use multi_throws::*;
use sdk::*;

pub static TEST_NONCE: sdk::SHA256Hash =
    sdk::sha256_static!("MULTI_THROWS_TEST_NONCE");

pub fn multi_throws_test(contract_hash: &SHA256Hash) {
    let addr = deploy_contract(&TEST_NONCE, contract_hash);
    let mut client = MultiClient::new(&addr);

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
            println!("OK: unknown error with selector 0x{:016X}", raw.selector());
            // The caller knows about SecretError and can try to decode it
            let secret = raw.try_as::<SecretError>();
            assert_eq!(secret, Some(SecretError));
            println!("OK: try_as decoded SecretError successfully");
        }
        other => panic!("expected Unknown, got {:?}", other),
    }
}
