#![allow(unused)]

use exn_catch::*;
use thruput_sdk::*;

pub static TEST_NONCE: thruput_sdk::SHA256Hash =
    thruput_sdk::sha256_static!("EXN_CATCH_TEST_NONCE");

pub static TEST_NONCE_2: thruput_sdk::SHA256Hash =
    thruput_sdk::sha256_static!("EXN_CATCH_TEST_NONCE_2");

fn check_classification(classify: impl Fn(u32) -> u32) {
    for (which, expected) in [0, 2, 3, 4, 5, 6, 1, 7].into_iter().enumerate() {
        assert_eq!(classify(which as u32), expected, "case {which}");
    }
}

pub fn exn_catch_test(contract_hash: &SHA256Hash) {
    let caller = deploy_contract(&TEST_NONCE, contract_hash);
    let target = deploy_contract(&TEST_NONCE_2, contract_hash);
    let client = ExnCatchClient::new(&caller);
    check_classification(|which| client.classify(target, which));
}

pub fn exn_catch_mapped_test(contract_hash: &SHA256Hash) {
    let caller = deploy_contract(&TEST_NONCE, contract_hash);
    let target = deploy_contract(&TEST_NONCE_2, contract_hash);
    let client = ExnCatchClient::new(&caller);
    check_classification(|which| client.classify_mapped(target, which));

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
