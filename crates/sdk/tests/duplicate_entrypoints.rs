//! Duplicate entrypoint IDs must fail loudly on the host path.
//!
//! Within one `#[contract]` block a collision is a macro-expansion error,
//! and any `cargo build` of a crate with colliding blocks fails at codegen
//! on the crate-scoped guard symbols (see
//! `fixtures/dup_entrypoint_neg`). This test covers the layer
//! underneath both: hand-written mock registrations that reuse a dispatch
//! ID must panic at test setup rather than silently shadowing the earlier
//! method, which is what an on-chain first-match dispatch would never do.

use thruput_sdk::contract::ContractEntrypointID;
use thruput_sdk::*;

#[test]
#[should_panic(expected = "duplicate entrypoint registration")]
fn duplicate_mock_method_registration_panics() {
    let env = TestEnvironment::no_cost();
    let _mock = env.register_mock(|c| {
        c.register_method(ContractEntrypointID::from_u64(0xD0D0), || {});
        c.register_method(ContractEntrypointID::from_u64(0xD0D0), || {});
    });
}

/// A rejected duplicate must not touch the dispatch map: a harness that
/// catches the panic and keeps using the mock still gets the ORIGINAL
/// handler, never the rejected replacement.
#[test]
fn rejected_duplicate_leaves_original_handler_installed() {
    const CHILD_ADDR: ContractAddress = ContractAddress::new([0x61; 32]);
    const PARENT_ADDR: ContractAddress = ContractAddress::new([0x62; 32]);
    let id = ContractEntrypointID::from_u64(0xACE);

    let env = TestEnvironment::no_cost();
    let mock = env.register_mock(|c| {
        c.register_method(id, || {
            thruput_sdk::internal::contract_return_encoded(&1u32)
        });
        let dup =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                c.register_method(id, || {
                    thruput_sdk::internal::contract_return_encoded(&2u32)
                });
            }));
        assert!(dup.is_err(), "duplicate registration must panic");
    });
    mock.deploy_contract_to(&CHILD_ADDR);

    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(move || {
            let returned: u32 = thruput_sdk::call::try_call_raw_with(
                &CHILD_ADDR,
                id,
                u64::MAX,
                &(),
                0,
                |exn| panic!("child aborted: {exn:?}"),
            )
            .expect("call through the surviving handler must succeed");
            assert_eq!(
                returned, 1,
                "original registration must remain installed"
            );
        })
    })
    .assert_success();
}

#[test]
fn distinct_mock_method_registrations_are_fine() {
    let env = TestEnvironment::no_cost();
    let _mock = env.register_mock(|c| {
        c.register_method(ContractEntrypointID::from_u64(1), || {});
        c.register_method(ContractEntrypointID::from_u64(2), || {});
    });
}
