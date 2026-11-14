//! The contract on the host, against host arithmetic. This checks the
//! contract's own dispatch; the rv32 routines are not part of a native
//! build.

mod common;
use common::*;
use int_division::IntDivisionClient;

#[test]
fn native_methods_match_host_arithmetic() {
    thruput_sdk::client_test(
        int_division::register_mock_contract,
        |c: &IntDivisionClient| {
            for kind in KINDS {
                let (a, b) = (0xFEDC_BA98_7654_3210, 0x1_0000_0001);
                assert_eq!(Some(c.op64(kind, a, b, 2)), op64(kind, a, b));
                let (a, b) = (u128::MAX / 3, 1_000_000_000_000_000_000);
                assert_eq!(Some(c.op128(kind, a, b, 2)), op128(kind, a, b));
            }
        },
    );
}
