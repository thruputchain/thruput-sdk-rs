//! Duplicate entrypoint IDs must fail loudly on the host path.
//!
//! Within one `#[contract]` block a collision is a macro-expansion error,
//! and any `cargo build` of a crate with colliding blocks fails at codegen
//! on the crate-scoped guard symbols (see
//! `example-contracts/dup_entrypoint_neg`). This test covers the layer
//! underneath both: hand-written mock registrations that reuse a dispatch
//! ID must panic at test setup rather than silently shadowing the earlier
//! method, which is what an on-chain first-match dispatch would never do.

use sdk::contract::ContractEntrypointID;
use sdk::*;

#[test]
#[should_panic(expected = "duplicate entrypoint registration")]
fn duplicate_mock_method_registration_panics() {
    let env = TestEnvironment::no_cost();
    let _mock = env.register_mock(|c| {
        c.register_method(ContractEntrypointID::from_u64(0xD0D0), || {});
        c.register_method(ContractEntrypointID::from_u64(0xD0D0), || {});
    });
}

#[test]
fn distinct_mock_method_registrations_are_fine() {
    let env = TestEnvironment::no_cost();
    let _mock = env.register_mock(|c| {
        c.register_method(ContractEntrypointID::from_u64(1), || {});
        c.register_method(ContractEntrypointID::from_u64(2), || {});
    });
}
