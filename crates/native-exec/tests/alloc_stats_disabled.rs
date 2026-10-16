//! Without the opt-in allocator, unavailable measurements are not zero.

use thruput_native_exec::test_environment::TestEnvironment;
use thruput_xdr::contract::ContractEntrypointID;
use thruput_xdr::invoke_args::{CopyableInvokeInputs, CopyableInvokeOutputs};
use thruput_xdr::transaction::InvocationFrameStatusCode;
use thruput_xdr::types::ContractAddress;

#[test]
fn measurements_are_unavailable_without_the_counting_allocator() {
    let env = TestEnvironment::no_cost();
    let child = ContractAddress::new([2; 32]);
    env.register_mock(|c| {
        c.register_method(ContractEntrypointID::from_u64(1), || {
            std::hint::black_box(vec![0u8; 4096]);
        });
    })
    .deploy_contract_to(&child);
    let result = env.transact(|tx| {
        tx.call(&ContractAddress::new([1; 32])).run(|| {
            let mut outputs = CopyableInvokeOutputs {
                return_buffer_len: 0,
                returned_auth_token_count: 0,
                frame_status: InvocationFrameStatusCode::Fault,
            };
            thruput_xdr::syscalls::raw_Invoke(
                &child,
                &[],
                &[],
                &CopyableInvokeInputs {
                    entrypoint_id: 1,
                    gas_limit: u64::MAX,
                    flags: 0,
                    reserved: 0,
                },
                &mut outputs,
            )
            .unwrap();
            assert_eq!(
                outputs.frame_status,
                InvocationFrameStatusCode::Return
            );
        })
    });
    result.assert_success();
    assert_eq!(result.frame().alloc_stats(), None);
    assert!(result.frame().cross_call_alloc_records().is_empty());
}
