#![allow(unused)]

use exn_catch::*;
use thruput_sdk::*;

pub static TEST_NONCE: thruput_sdk::SHA256Hash =
    thruput_sdk::sha256_static!("EXN_CATCH_TEST_NONCE");

pub static TEST_NONCE_2: thruput_sdk::SHA256Hash =
    thruput_sdk::sha256_static!("EXN_CATCH_TEST_NONCE_2");

/// Drive `classify` across every outcome, so the guest-side dispatch is
/// actually exercised rather than merely compiled.
pub fn exn_catch_test(contract_hash: &SHA256Hash) {
    let caller = deploy_contract(&TEST_NONCE, contract_hash);
    let target = deploy_contract(&TEST_NONCE_2, contract_hash);
    let client = ExnCatchClient::new(&caller);

    assert_eq!(client.classify(target, 0), 0, "success");
    assert_eq!(client.classify(target, 1), 2, "AlphaError::NotFound");
    assert_eq!(client.classify(target, 2), 3, "AlphaError::Overflow");
    assert_eq!(client.classify(target, 3), 4, "BetaError::Denied");
    assert_eq!(client.classify(target, 4), 5, "Undeclared (not in throws)");
    assert_eq!(client.classify(target, 5), 6, "LibraryExn");
    assert_eq!(client.classify(target, 6), 1, "fault");
    assert_eq!(client.classify(target, 7), 7, "malformed abort");

    client.classify_then_throw(target, 0);
    for which in 1..=7 {
        let error = client.try_classify_then_throw(target, which).unwrap_err();
        assert!(matches!(
            error.try_as::<AlphaError>(),
            Some(AlphaError::Overflow)
        ));
    }
    let error = client.try_rethrow(target).unwrap_err();
    assert!(error.try_as::<Undeclared>().is_some());
}
