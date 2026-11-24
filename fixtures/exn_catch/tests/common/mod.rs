#![allow(unused)]

use exn_catch::*;
use thruput_sdk::*;

pub static TEST_NONCE: DeployNonce =
    DeployNonce::new(thruput_sdk::sha256_static!("EXN_CATCH_TEST_NONCE"));

pub static TEST_NONCE_2: DeployNonce =
    DeployNonce::new(thruput_sdk::sha256_static!("EXN_CATCH_TEST_NONCE_2"));

fn check_classification(classify: impl Fn(u32) -> u32) {
    for (which, expected) in [0, 2, 3, 4, 5, 6, 1, 7].into_iter().enumerate() {
        assert_eq!(classify(which as u32), expected, "case {which}");
    }
}

pub fn exn_catch_test(code: &CodeHash<ExnCatchClient>) {
    let client = code.deploy(&TEST_NONCE).client();
    let target = *code.deploy(&TEST_NONCE_2);
    check_classification(|which| client.classify(target, which));
}

pub fn exn_catch_mapped_test(code: &CodeHash<ExnCatchClient>) {
    let client = code.deploy(&TEST_NONCE).client();
    let target = *code.deploy(&TEST_NONCE_2);
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
