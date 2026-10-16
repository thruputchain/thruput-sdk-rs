//! `contract_return_encoded` is the *successful*-return encode path. When the
//! return value fails to encode, the abort must be reported as
//! `HandlingError::EncodingSuccess` — not `EncodingException`, which is for a
//! failed *exception* (abort) encode. Regression test for the swapped
//! discriminant: a callee that mislabels the failure would make the caller
//! mis-decode why the call failed.

mod common;
use common::decode_library_exn;

use thruput_sdk::call::RawCallError;
use thruput_sdk::contract::ContractEntrypointID;
use thruput_sdk::exn::Exn;
use thruput_sdk::{
    AuthHandleBuf, CollectAuthHandles, ContractAddress, FaultString,
    HandlingError, LibraryExn, TestEnvironment, TestEnvironmentHandle,
    WriteError, WriteResult, WriteXDR, Writer,
};

const CHILD_ADDR: ContractAddress = ContractAddress::new([0x73; 32]);
const PARENT_ADDR: ContractAddress = ContractAddress::new([0x71; 32]);

fn entrypoint() -> ContractEntrypointID {
    ContractEntrypointID::from_u64(0x0BAD_0BAD_0BAD_0BAD)
}

/// A return type whose encoding always fails and that carries no auth handles,
/// so `contract_return_encoded` clears auth-handle collection and then fails at
/// the encode step.
struct FailToEncode;

impl WriteXDR for FailToEncode {
    fn write_xdr<F, W: Writer<F>>(&self, _w: &mut W) -> WriteResult<()> {
        Err(WriteError::InvalidArrayLength)
    }

    unsafe fn write_xdr_unchecked<
        F,
        W: thruput_sdk::xdr::WriterUnchecked<F>,
    >(
        &self,
        _w: &mut W,
    ) {
        // Checked-encode failure has no unchecked analogue; this type only
        // exercises the fallible return path.
    }
}

impl CollectAuthHandles for FailToEncode {
    fn collect_auth_handles(
        &self,
        _out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString> {
        Ok(())
    }
}

#[test]
fn failed_return_encode_reports_encoding_success_not_exception() {
    let env: TestEnvironmentHandle = TestEnvironment::no_cost();
    let mock = env.register_mock(|c| {
        c.register_method(entrypoint(), || {
            thruput_sdk::internal::contract_return_encoded(&FailToEncode);
        });
    });
    mock.deploy_contract_to(&CHILD_ADDR);

    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(|| {
            let err = thruput_sdk::call::try_call_raw_with::<
                (),
                (),
                thruput_sdk::exn::RawException,
            >(
                &CHILD_ADDR,
                entrypoint(),
                u64::MAX,
                &(),
                0,
                |exn| exn.to_owned(),
            )
            .expect_err("child must abort: its return value fails to encode");

            match err {
                RawCallError::Exception(exn)
                    if exn.selector() == LibraryExn::SELECTOR =>
                {
                    match decode_library_exn(exn.payload()) {
                        LibraryExn::Handling(
                            HandlingError::EncodingSuccess(_),
                        ) => {}
                        other => panic!(
                            "expected Handling(EncodingSuccess), got {other:?}"
                        ),
                    }
                }
                other => {
                    panic!("expected LibraryExn exception, got {other:?}")
                }
            }
        })
    })
    .assert_success();
}
