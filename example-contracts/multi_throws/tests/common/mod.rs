#![allow(unused)]

use multi_throws::*;
use sdk::*;

pub static TEST_NONCE: sdk::SHA256Hash =
    sdk::sha256_static!("MULTI_THROWS_TEST_NONCE");

pub static TEST_NONCE_2: sdk::SHA256Hash =
    sdk::sha256_static!("MULTI_THROWS_TEST_NONCE_2");

/// One place that handles every exception this contract can produce.
///
/// `SecretError` is *not* in the interface's `throws(AlphaError, BetaError)`
/// list, and needs nothing special to match: exceptions carry a selector
/// identifying their type independently of what any interface declared, so
/// a declared error and an undeclared one are peers here.
fn classify(e: &CallError) -> &'static str {
    match_exn!(e, {
        AlphaError::NotFound => "alpha-not-found",
        AlphaError::Overflow => "alpha-overflow",
        BetaError::Denied => "beta-denied",
        SecretError => "secret",
        _ => "other",
    })
}

pub fn multi_throws_test(contract_hash: &SHA256Hash) {
    let addr = deploy_contract(&TEST_NONCE, contract_hash);
    let client = MultiClient::new(&addr);

    // Test 1: Success path
    assert_eq!(client.try_succeed(), Ok(42));
    assert_eq!(client.succeed(), 42);

    // Test 2/3/4: declared errors dispatch to their own arms.
    assert_eq!(
        classify(&client.try_throw_alpha_not_found().unwrap_err()),
        "alpha-not-found"
    );
    assert_eq!(
        classify(&client.try_throw_alpha_overflow().unwrap_err()),
        "alpha-overflow"
    );
    assert_eq!(
        classify(&client.try_throw_beta_denied().unwrap_err()),
        "beta-denied"
    );

    // Test 5: an undeclared error matches by the same route as a declared
    // one — no `Unknown` detour, no manual decode step.
    assert_eq!(
        classify(&client.try_throw_undeclared().unwrap_err()),
        "secret"
    );
    // And the single-type shorthand reaches it just as directly.
    assert_eq!(
        client.try_throw_undeclared().unwrap_err().try_as(),
        Some(SecretError)
    );

    // Test 6: Error forwarding through a call chain.
    // Deploy a second instance; call forward_alpha on the first, which
    // calls throw_alpha_not_found on the second via the non-try method.
    // re_throw should forward the error so we see AlphaError here.
    let addr2 = deploy_contract(&TEST_NONCE_2, contract_hash);
    assert_eq!(
        classify(&client.try_forward_alpha(addr2.clone()).unwrap_err()),
        "alpha-not-found"
    );

    // Test 7: Forwarding an undeclared error through a call chain. The
    // inner contract throws SecretError and the middle contract re-throws;
    // the selector survives both hops.
    assert_eq!(
        classify(&client.try_forward_undeclared(addr2.clone()).unwrap_err()),
        "secret"
    );

    // Test 8: Forwarding a fault through a call chain. The inner contract
    // panics and the middle contract's non-try method propagates it as a
    // panic. A fault carries no exception, so it stays an ordinary `match`
    // arm rather than a `match_exn!` one.
    match client.try_forward_fault(addr2) {
        Err(CallError::Fault) => {
            println!("OK: forwarded fault through call chain");
        }
        other => panic!("expected forwarded Fault, got {:?}", other),
    }
}

/// An exception tagged with a *declared* selector whose payload does not
/// decode as that type.
///
/// There is no distinct `BadDecode` variant for this: `try_as` returns
/// `None` both for a selector that matches nothing and for one that matches
/// a type the payload cannot produce, so `match_exn!` sweeps it into `_`.
/// `try_decode` is what tells the two apart, and this pins both halves —
/// the fall-through and the report.
pub fn multi_throws_test_undecodable(contract_hash: &SHA256Hash) {
    let addr = deploy_contract(&TEST_NONCE, contract_hash);
    let client = MultiClient::new(&addr);

    match client.try_throw_undecodable() {
        Err(ref e @ CallError::Exception(ref raw)) => {
            assert_eq!(raw.selector(), multi_throws::AlphaError::SELECTOR);
            // Right selector, and still undecodable as that type. `try_as`
            // cannot distinguish that from an unrelated error type, so
            // `match_exn!` sweeps it into `_`; `try_decode` reports it.
            assert_eq!(raw.try_as::<AlphaError>(), None);
            assert!(matches!(e.try_decode::<AlphaError>(), Some(Err(_))));
            assert_eq!(
                match_exn!(e, {
                    AlphaError::NotFound | AlphaError::Overflow => "alpha",
                    _ => "wildcard",
                }),
                "wildcard"
            );
            let decoded: u32 = sdk::xdr::cxs::decode(raw.payload()).unwrap();
            assert_eq!(decoded, 42);
            println!("OK: undecodable payload under a declared selector");
        }
        other => panic!("expected a caught exception, got {:?}", other),
    }
}

pub fn multi_throws_test_relay_preserves_payload(contract_hash: &SHA256Hash) {
    let addr = deploy_contract(&TEST_NONCE, contract_hash);
    let addr2 = deploy_contract(&TEST_NONCE_2, contract_hash);
    let client = MultiClient::new(&addr);

    match client.try_relay_undecodable(addr2) {
        Err(CallError::Exception(raw)) => {
            // The re-thrown wire bytes must be verbatim: same selector,
            // same undecodable payload (42u32) as calling
            // `throw_undecodable` directly, even though the contract
            // performed a scratch-reusing op (`42u32.hash()`) between
            // catching the exception and re-throwing it. Guards against a
            // future regression where the detach copy is skipped.
            assert_eq!(raw.selector(), multi_throws::AlphaError::SELECTOR);
            let decoded: u32 = sdk::xdr::cxs::decode(raw.payload()).unwrap();
            assert_eq!(decoded, 42);
            println!("OK: relay preserved selector and payload verbatim");
        }
        other => panic!("expected a caught exception, got {:?}", other),
    }
}

pub fn multi_throws_test_relay_payload_readable_after_interleave(
    contract_hash: &SHA256Hash,
) {
    let addr = deploy_contract(&TEST_NONCE, contract_hash);
    let addr2 = deploy_contract(&TEST_NONCE_2, contract_hash);
    let client = MultiClient::new(&addr);

    // Same relay path, but here we assert observe-then-decide: the caught
    // exception's payload must still be readable (not just re-throwable)
    // after the interleaved scratch-reusing op inside the contract.
    match client.try_relay_undecodable(addr2) {
        Err(CallError::Exception(raw)) => {
            let payload_len = raw.payload().len();
            assert!(payload_len > 0, "payload must be non-empty");
            let decoded: u32 = sdk::xdr::cxs::decode(raw.payload()).unwrap();
            assert_eq!(decoded, 42);
            // Read it again to confirm the slot wasn't consumed/cleared by
            // the first read.
            let decoded_again: u32 =
                sdk::xdr::cxs::decode(raw.payload()).unwrap();
            assert_eq!(decoded_again, 42);
            println!(
                "OK: caught exception payload readable after interleaved op"
            );
        }
        other => panic!("expected a caught exception, got {:?}", other),
    }
}

pub fn multi_throws_test_relay_undecodable_twice(contract_hash: &SHA256Hash) {
    let addr = deploy_contract(&TEST_NONCE, contract_hash);
    let addr2 = deploy_contract(&TEST_NONCE_2, contract_hash);
    let client = MultiClient::new(&addr);

    // The contract catches two undecodable exceptions and holds both alive
    // simultaneously (asserting both decode correctly across an
    // interleaved scratch-reusing op), then re-throws the first. Pins the
    // owner-directed guarantee: no-alloc catching is unlimited/concurrent,
    // identical to `alloc` — each caught exception owns its buffer, so any
    // number may be live concurrently.
    match client.try_relay_undecodable_twice(addr2) {
        Err(CallError::Exception(raw)) => {
            assert_eq!(raw.selector(), multi_throws::AlphaError::SELECTOR);
            let decoded: u32 = sdk::xdr::cxs::decode(raw.payload()).unwrap();
            assert_eq!(decoded, 42);
            println!(
                "OK: two concurrently-held caught exceptions both survived interleave; outer re-throw verbatim"
            );
        }
        other => panic!("expected a caught exception, got {:?}", other),
    }
}

/// A contract may abort with the SDK's own `LibraryExn` selector and a
/// payload that is not a `LibraryExn`. That must not be mistaken for a real
/// SDK error: `LibraryExn` is matched by the same selector-then-decode
/// route as any other exception type, so the decode failure keeps it out of
/// every typed arm.
pub fn multi_throws_test_malicious_library_error(contract_hash: &SHA256Hash) {
    let addr = deploy_contract(&TEST_NONCE, contract_hash);
    let client = MultiClient::new(&addr);

    match client.try_throw_malicious_library_error() {
        Err(ref e @ CallError::Exception(ref raw)) => {
            assert_eq!(raw.selector(), LibraryExn::SELECTOR);
            assert_eq!(raw.try_as::<LibraryExn>(), None);
            // A `LibraryExn` arm does not catch it.
            assert_eq!(
                match_exn!(e, {
                    LibraryExn::NonNegativeIntOverflow => "overflow",
                    _ => "unmatched",
                }),
                "unmatched"
            );
            let decoded: u32 = sdk::xdr::cxs::decode(raw.payload()).unwrap();
            assert_eq!(decoded, 42);
            println!("OK: forged LibraryExn payload matched no typed arm");
        }
        other => panic!("expected a caught exception, got {:?}", other),
    }
}
