//! Cross-contract calls whose return/abort payloads exceed the legacy
//! 500-byte buffers. With `alloc`, buffers are sized from the lengths the
//! `Invoke` syscall reports (up to the protocol's 1024-byte cap).
//!
//! The mock children return raw bytes via `raw_Return`, bypassing the
//! SDK's (still 500-byte-capped) encode path, which is out of scope here.

use sdk::call::RawCallError;
use sdk::contract::ContractEntrypointID;
use sdk::{
    AuthHandleBuf, ContractAddress, TestEnvironment, TestEnvironmentHandle,
};

const CHILD_ADDR: ContractAddress = ContractAddress::new([0x42; 32]);
const PARENT_ADDR: ContractAddress = ContractAddress::new([0x41; 32]);

fn entrypoint() -> ContractEntrypointID {
    ContractEntrypointID::from_u64(0x1122_3344_5566_7788)
}

fn env_with_child(
    child_body: impl Fn() + Clone + std::panic::UnwindSafe + 'static,
) -> TestEnvironmentHandle {
    let env = TestEnvironment::no_cost();
    let mock = env.register_mock(|c| {
        c.register_method(entrypoint(), child_body);
    });
    mock.deploy_contract_to(&CHILD_ADDR);
    env
}

#[test]
fn contract_call_receives_returns_larger_than_500_bytes() {
    let env = env_with_child(|| {
        let payload = [0xAB_u8; 700];
        sdk::sys::raw_Return(&payload, false, &[]);
    });

    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(|| {
            let result = sdk::call::contract_call(
                &CHILD_ADDR,
                entrypoint(),
                u64::MAX,
                &[] as &[u8],
                &AuthHandleBuf::new(),
                0,
            )
            .expect("invoke failed")
            .expect("child aborted");
            assert_eq!(result.0.as_slice(), &[0xAB_u8; 700][..]);
        })
    })
    .assert_success();
}

#[test]
fn try_call_raw_decodes_returns_larger_than_500_bytes() {
    // The child returns the CXS encoding of a 600-byte Vec<u8>.
    let encoded =
        sdk::xdr::cxs::encode(&vec![0xCD_u8; 600]).expect("encode failed");
    let env = env_with_child(move || {
        sdk::sys::raw_Return(&encoded, false, &[]);
    });

    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(|| {
            // Call twice: the second call exercises scratch-buffer reuse.
            for _ in 0..2 {
                let result: Vec<u8> = sdk::call::try_call_raw(
                    &CHILD_ADDR,
                    entrypoint(),
                    u64::MAX,
                    &(),
                    0,
                )
                .expect("call failed");
                assert_eq!(result, vec![0xCD_u8; 600]);
            }
        })
    })
    .assert_success();
}

#[test]
fn try_call_raw_preserves_abort_payloads_larger_than_500_bytes() {
    // Abort buffer layout: [u64 selector][payload bytes...].
    let selector: u64 = 0xDEAD_BEEF_CAFE_F00D;
    let mut abort_buf =
        sdk::xdr::cxs::encode(&selector).expect("encode failed");
    abort_buf.extend_from_slice(&[0xEF_u8; 700]);
    let env = env_with_child(move || {
        sdk::sys::raw_Return(&abort_buf, true, &[]);
    });

    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(|| {
            let err = sdk::call::try_call_raw::<(), Vec<u8>>(
                &CHILD_ADDR,
                entrypoint(),
                u64::MAX,
                &(),
                0,
            )
            .expect_err("child must abort");
            match err {
                RawCallError::Exception(exn) => {
                    assert_eq!(exn.selector(), selector);
                    // The payload must arrive complete, not truncated to
                    // RawException's legacy 500-byte cap.
                    assert_eq!(exn.payload(), &[0xEF_u8; 700][..]);
                }
                other => panic!("expected Exception, got {other:?}"),
            }
        })
    })
    .assert_success();
}
