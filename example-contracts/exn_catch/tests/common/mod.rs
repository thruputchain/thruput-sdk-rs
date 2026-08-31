#![allow(unused)]

use exn_catch::*;
use sdk::*;

pub static TEST_NONCE: sdk::SHA256Hash =
    sdk::sha256_static!("EXN_CATCH_TEST_NONCE");

pub static TEST_NONCE_2: sdk::SHA256Hash =
    sdk::sha256_static!("EXN_CATCH_TEST_NONCE_2");

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
}
