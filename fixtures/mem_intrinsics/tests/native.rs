//! The contract on the host, against the model in `common`. This checks the
//! model and the contract's own indexing; the rv32 routines are not part of
//! a native build.

mod common;
use mem_intrinsics::MemIntrinsicsClient;

#[test]
fn native_methods_match_model() {
    thruput_sdk::client_test(
        mem_intrinsics::register_mock_contract,
        |c: &MemIntrinsicsClient| {
            for len in [0, 1, 4, 31, 32, 33, 100, 400] {
                assert_eq!(c.copy(len, 1, 2), common::copy(len, 1, 2));
                assert_eq!(c.shift(len, 40, 37), common::shift(len, 40, 37));
                assert_eq!(c.shift(len, 40, 43), common::shift(len, 40, 43));
                assert_eq!(c.fill(len, 3, 0x5A), common::fill(len, 3, 0x5A));
            }
        },
    );
}
