//! Calling a method this contract does not define must come back as the
//! typed `LibraryExn::UnknownMethod`, not as an undifferentiated
//! deterministic failure.

use thruput_sdk::call::RawCallError;
use thruput_sdk::contract::ContractEntrypointID;
use thruput_sdk::exn::Exn;
use thruput_sdk::*;
use unknown_method::DefinedClient;

static NONCE: SHA256Hash = thruput_sdk::sha256_static!("UNKNOWN_METHOD_TEST_NONCE");

const CALLER: ContractAddress = ContractAddress::new([0x71; 32]);

/// An entrypoint id the contract cannot possibly define: entrypoint ids are
/// truncated hashes of (interface, version, signature), so a hand-picked
/// constant is unreachable by construction.
fn absent() -> ContractEntrypointID {
    ContractEntrypointID::from_u64(0x0BAD_0BAD_0BAD_0BAD)
}

#[test]
fn undefined_method_throws_unknown_method() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let contract = unknown_method::register_mock_contract(&env);
    let contract_hash = *contract.as_hash();

    env.transact(|tx| {
        tx.call(&CALLER).run(move || {
            let addr = deploy_contract(&NONCE, &contract_hash);

            // The defined method still works.
            assert_eq!(DefinedClient::new(&addr).defined(), 1);

            let err = thruput_sdk::call::try_call_raw_with::<(), (), thruput_sdk::exn::RawException>(
                &addr,
                absent(),
                u64::MAX,
                &(),
                0,
                |exn| exn.to_owned(),
            )
            .expect_err("an undefined method must abort the call");

            match err {
                RawCallError::Exception(exn)
                    if exn.selector() == LibraryExn::SELECTOR =>
                {
                    assert_eq!(
                        exn.try_as::<LibraryExn>(),
                        Some(LibraryExn::UnknownMethod),
                    );
                }
                other => {
                    panic!("expected LibraryExn::UnknownMethod, got {other:?}")
                }
            }
        })
    })
    .assert_success();
}
