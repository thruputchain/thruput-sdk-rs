//! The contract against the host allocator. src/main.rs, and with it the
//! custom allocator, is not part of a native build.

mod common;
use common::*;
use custom_allocator::CustomAllocatorClient;

#[test]
fn native_methods() {
    thruput_sdk::client_test(
        |env| custom_allocator::register_mock_contract(env),
        |c: &CustomAllocatorClient| {
            assert_eq!(c.heap_bytes(), 0);
            assert_eq!(c.churn(3, 1000), churn_sum(3, 1000));
            assert_eq!(c.grow(1000), grow_sum(1000));
            assert_eq!(c.grow_below(1000, 1000, 1000), 2000);
            assert!(!c.grow_fail(1000, 1000, true));
        },
    );
}
