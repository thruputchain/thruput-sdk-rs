//! End-to-end tests for per-frame allocation stats.
//!
//! Assertions use `>=` / `<` bounds rather than exact counts: the
//! measured windows legitimately include small harness allocations, and
//! exact numbers would be brittle across std/compiler versions.

groundhog_native_exec::install_counting_allocator!();

use groundhog_native_exec::alloc_stats::drain_frame_records;
use groundhog_native_exec::frame_builder::FrameAction;
use groundhog_native_exec::test_environment::TestEnvironment;
use groundhog_xdr::contract::ContractEntrypointID;
use groundhog_xdr::transaction::InvocationFrameStatusCode;
use groundhog_xdr::types::ContractAddress;
use std::sync::atomic::{AtomicU64, Ordering};

const PARENT_ADDR: ContractAddress = ContractAddress::new([0x11; 32]);
const CHILD_ADDR: ContractAddress = ContractAddress::new([0x22; 32]);

/// Cross-call the contract at `addr` the way `client.foo()` does: via the
/// `Invoke` syscall, which routes through hogd and back into the
/// registered mock method.
fn invoke_mock(addr: &ContractAddress, entrypoint: ContractEntrypointID) {
    use groundhog_xdr::invoke_args::{
        CopyableInvokeInputs, CopyableInvokeOutputs,
    };
    use groundhog_xdr::syscalls as sys;
    let inputs = CopyableInvokeInputs {
        entrypoint_id: u64::from_be_bytes(entrypoint.into_inner()),
        gas_limit: u64::MAX,
        flags: 0,
    };
    let mut outputs = CopyableInvokeOutputs {
        return_buffer_len: 0,
        returned_auth_token_len: 0,
        frame_status: InvocationFrameStatusCode::Fault,
    };
    sys::raw_Invoke(addr, &[], &[], &inputs, &mut outputs)
        .expect("Invoke syscall failed");
    assert_eq!(outputs.frame_status, InvocationFrameStatusCode::Return);
}

#[test]
fn root_frame_reports_contract_allocations() {
    let env = TestEnvironment::no_cost();
    let result = env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(|| {
            let v: Vec<u8> = Vec::with_capacity(4096);
            std::hint::black_box(&v);
        })
    });
    result.assert_success();

    let stats = result.frame().alloc_stats();
    assert!(
        stats.bytes_total >= 4096,
        "the 4 KiB Vec must be counted, got {stats:?}"
    );
    assert!(stats.allocs >= 1, "got {stats:?}");
    assert!(
        stats.heap_used >= stats.bytes_total,
        "bump-cursor consumption includes padding, got {stats:?}"
    );
}

#[test]
fn child_frame_allocations_are_charged_to_the_child_not_the_parent() {
    const CHILD_ALLOC: usize = 64 * 1024;
    static CHILD_BYTES: AtomicU64 = AtomicU64::new(0);

    let env = TestEnvironment::no_cost();
    let result = env.transact(|tx| {
        tx.call(&PARENT_ADDR)
            .run_continuable(|frame| {
                let child_result = frame.child_call(&CHILD_ADDR).run(|| {
                    let v = vec![0u8; CHILD_ALLOC];
                    std::hint::black_box(&v);
                });
                child_result.assert_success();
                CHILD_BYTES.store(
                    child_result.alloc_stats().bytes_total,
                    Ordering::Relaxed,
                );

                let v = vec![0u8; 512];
                std::hint::black_box(&v);
                FrameAction::Close
            })
            .finish()
    });
    result.assert_success();

    let child_bytes = CHILD_BYTES.load(Ordering::Relaxed);
    assert!(
        child_bytes >= CHILD_ALLOC as u64,
        "child frame must be charged its 64 KiB Vec, got {child_bytes}"
    );

    let parent = result.frame().alloc_stats();
    assert!(
        parent.bytes_total >= 512,
        "parent frame must see its own Vec, got {parent:?}"
    );
    assert!(
        parent.bytes_total < CHILD_ALLOC as u64 / 2,
        "parent frame must NOT be charged the child's 64 KiB \
         (each frame is its own guest heap), got {parent:?}"
    );
}

#[test]
fn cross_call_frames_produce_labeled_alloc_records() {
    const MOCK_ALLOC: usize = 32 * 1024;
    let entrypoint = ContractEntrypointID::from_u64(0xAABB_CCDD_EEFF_0011);

    let env = TestEnvironment::no_cost();
    let mock = env.register_mock(|c| {
        c.register_method(entrypoint, || {
            let v = vec![0u8; MOCK_ALLOC];
            std::hint::black_box(&v);
        });
    });
    mock.deploy_contract_to(&CHILD_ADDR);

    // Clear anything earlier activity on this thread left behind.
    drain_frame_records();

    let result = env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(|| {
            invoke_mock(&CHILD_ADDR, entrypoint);
        })
    });
    result.assert_success();

    let records = drain_frame_records();
    assert_eq!(
        records.len(),
        1,
        "one cross-call frame must produce one record, got {records:?}"
    );
    assert_eq!(records[0].entrypoint, entrypoint);
    assert_eq!(records[0].mock_contract_index, 0);
    assert!(
        records[0].stats.bytes_total >= MOCK_ALLOC as u64,
        "callee frame must be charged its 32 KiB Vec, got {:?}",
        records[0].stats
    );

    let caller = result.frame().alloc_stats();
    assert!(
        caller.bytes_total < MOCK_ALLOC as u64 / 2,
        "caller frame must NOT be charged the callee's allocation, \
         got {caller:?}"
    );
}

#[test]
fn stats_accumulate_across_continue_steps() {
    let env = TestEnvironment::no_cost();
    let result = env.transact(|tx| {
        tx.call(&PARENT_ADDR)
            .run_continuable(|_| {
                let v = vec![0u8; 2048];
                std::hint::black_box(&v);
                FrameAction::Continue
            })
            .run_continuable(|_| {
                let v = vec![0u8; 4096];
                std::hint::black_box(&v);
                FrameAction::Close
            })
            .finish()
    });
    result.assert_success();

    let stats = result.frame().alloc_stats();
    assert!(
        stats.bytes_total >= 2048 + 4096,
        "both execute steps of the frame must accumulate, got {stats:?}"
    );
}
