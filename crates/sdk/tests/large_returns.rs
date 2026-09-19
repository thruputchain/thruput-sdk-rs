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
    child_body: impl Fn() + Clone + std::panic::UnwindSafe + Send + Sync + 'static,
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
            let mut return_buf = sdk::call::ReturnBuf::new();
            let mut returned_auth = AuthHandleBuf::new();
            let outcome = sdk::call::contract_call(
                &CHILD_ADDR,
                entrypoint(),
                u64::MAX,
                &[] as &[u8],
                &AuthHandleBuf::new(),
                0,
                &mut return_buf,
                &mut returned_auth,
            )
            .expect("invoke failed");
            assert_eq!(outcome, sdk::call::CallOutcome::Return);
            assert_eq!(return_buf.as_slice(), &[0xAB_u8; 700][..]);
        })
    })
    .assert_success();
}

#[test]
fn try_call_raw_with_decodes_returns_larger_than_500_bytes() {
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
                let result: Vec<u8> = sdk::call::try_call_raw_with(
                    &CHILD_ADDR,
                    entrypoint(),
                    u64::MAX,
                    &(),
                    0,
                    |exn| exn.to_owned(),
                )
                .expect("call failed");
                assert_eq!(result, vec![0xCD_u8; 600]);
            }
        })
    })
    .assert_success();
}

#[test]
fn try_call_raw_with_preserves_abort_payloads_larger_than_500_bytes() {
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
            let err = sdk::call::try_call_raw_with::<(), Vec<u8>, _>(
                &CHILD_ADDR,
                entrypoint(),
                u64::MAX,
                &(),
                0,
                |exn| exn.to_owned(),
            )
            .expect_err("child must abort");
            match err {
                RawCallError::Exception(exn) => {
                    assert_eq!(exn.selector(), selector);
                    // The payload must arrive complete, not truncated to
                    // RawExceptionRef's legacy 500-byte cap.
                    assert_eq!(exn.payload(), &[0xEF_u8; 700][..]);
                }
                other => panic!("expected Exception, got {other:?}"),
            }
        })
    })
    .assert_success();
}

// `contract_call` also clears both out-buffers when a *parse* failure
// happens after `invoke_frame` succeeds: `resize_auth_handles` grows
// `returned_auth_token_ids` to its final length before `fill_return_buf`
// gets a chance to fail, and `fill_return_buf` itself sets `return_buf`'s
// length before its own trailing auth-handle-length check can fail (see
// the `contract_call` doc comment in `calls.rs`). Both were left dirty on
// those paths; the fix wraps the fallible section with an explicit clear
// on any `Err`.
//
// That path isn't exercised here: `Invoke`'s reported return/auth-token
// lengths and `GetReturnedBuffer`'s reported lengths are read from the
// same underlying per-frame state on the host side (only a subsequent
// `Return` can overwrite it), so no mock contract behavior can make them
// diverge and reach `checked_returned_len`'s or
// `validate_auth_token_id_list_len`'s error arms through a real call. Only
// the `RawCallError::Invalid` path below is mock-constructible; the fix's
// other two error arms are covered by code inspection and by
// `crate::call::calls::tests` unit-testing `checked_returned_len` as a
// pure function.
#[test]
fn contract_call_clears_buffers_even_when_the_call_fails() {
    // Fresh env: CHILD_ADDR is never registered, so `invoke_frame` rejects
    // the call before any child frame runs (`RawCallError::Invalid`).
    let env = TestEnvironment::no_cost();
    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(|| {
            let mut return_buf = sdk::call::ReturnBuf::new();
            return_buf.extend_from_slice(b"stale-from-previous-call");
            let mut returned_auth = AuthHandleBuf::new();
            returned_auth.push(7);
            let result = sdk::call::contract_call(
                &CHILD_ADDR,
                entrypoint(),
                u64::MAX,
                &[] as &[u8],
                &AuthHandleBuf::new(),
                0,
                &mut return_buf,
                &mut returned_auth,
            );
            assert!(matches!(result, Err(RawCallError::Invalid)));
            assert!(
                return_buf.is_empty(),
                "return_buf must be cleared on Err"
            );
            assert!(
                returned_auth.is_empty(),
                "auth buf must be cleared on Err"
            );
        })
    })
    .assert_success();
}
