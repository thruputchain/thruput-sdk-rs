//! Native allocation estimates exclude callees and host bookkeeping.

thruput_native_exec::install_counting_allocator!();

use std::alloc::{Layout, alloc, dealloc};
use thruput_native_exec::alloc_stats::AllocStats;
use thruput_native_exec::frame_builder::FrameAction;
use thruput_native_exec::test_environment::TestEnvironment;
use thruput_xdr::contract::ContractEntrypointID;
use thruput_xdr::invoke_args::{CopyableInvokeInputs, CopyableInvokeOutputs};
use thruput_xdr::transaction::InvocationFrameStatusCode;
use thruput_xdr::types::ContractAddress;

const ROOT: ContractAddress = ContractAddress::new([0x11; 32]);
const CHILD: ContractAddress = ContractAddress::new([0x22; 32]);
const LEAF: ContractAddress = ContractAddress::new([0x33; 32]);

fn invoke(addr: &ContractAddress, method: u64) -> InvocationFrameStatusCode {
    let inputs = CopyableInvokeInputs {
        entrypoint_id: method,
        gas_limit: u64::MAX,
        flags: 0,
        reserved: 0,
    };
    let mut outputs = CopyableInvokeOutputs {
        return_buffer_len: 0,
        returned_auth_token_count: 0,
        frame_status: InvocationFrameStatusCode::Fault,
    };
    thruput_xdr::syscalls::raw_Invoke(addr, &[], &[], &inputs, &mut outputs)
        .expect("Invoke syscall failed");
    outputs.frame_status
}

fn allocate(size: usize, align: usize) {
    let layout = Layout::from_size_align(size, align).unwrap();
    // Explicit allocation avoids Vec growth and return-value encoding in
    // the continuation-padding regression.
    unsafe {
        let ptr = alloc(layout);
        assert!(!ptr.is_null());
        std::hint::black_box(ptr);
        dealloc(ptr, layout);
    }
}

#[test]
fn continuation_preserves_padding_and_excludes_child_and_harness() {
    let env = TestEnvironment::no_cost();
    let result = env.transact(|tx| {
        tx.call(&ROOT)
            .run_continuable(|_| {
                allocate(1, 1);
                FrameAction::Continue
            })
            .run_continuable(|frame| {
                let child = frame
                    .child_call(&CHILD)
                    .run_continuable(|_| {
                        allocate(4096, 1);
                        FrameAction::Close
                    })
                    .finish();
                child.assert_success();
                assert_eq!(child.alloc_stats().unwrap().bytes_total, 4096);
                allocate(8, 8);
                FrameAction::Close
            })
            .finish()
    });
    result.assert_success();
    assert_eq!(
        result.frame().alloc_stats(),
        Some(AllocStats {
            allocs: 2,
            bytes_total: 9,
            estimated_heap_bytes: 16,
        })
    );
}

#[test]
fn nested_and_sibling_cross_calls_keep_exclusive_labeled_records() {
    let env = TestEnvironment::no_cost();
    let leaf = env.register_mock(|c| {
        c.register_method(ContractEntrypointID::from_u64(2), || {
            allocate(32768, 1)
        });
    });
    leaf.deploy_contract_to(&LEAF);
    let child = env.register_mock(|c| {
        c.register_method(ContractEntrypointID::from_u64(1), || {
            allocate(512, 1);
            assert_eq!(invoke(&LEAF, 2), InvocationFrameStatusCode::Return);
        });
    });
    child.deploy_contract_to(&CHILD);
    let result = env.transact(|tx| {
        tx.call(&ROOT).run(|| {
            assert_eq!(invoke(&CHILD, 1), InvocationFrameStatusCode::Return);
            assert_eq!(invoke(&LEAF, 2), InvocationFrameStatusCode::Return);
        })
    });
    result.assert_success();
    let records = result.frame().cross_call_alloc_records();
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].contract, leaf.as_contract());
    assert_eq!(records[0].entrypoint, ContractEntrypointID::from_u64(2));
    assert_eq!(records[0].stats.bytes_total, 32768);
    assert_eq!(records[1].contract, child.as_contract());
    assert_eq!(records[1].entrypoint, ContractEntrypointID::from_u64(1));
    assert_eq!(records[1].stats.bytes_total, 512);
    assert_eq!(records[2], records[0]);
    assert!(result.frame().alloc_stats().unwrap().bytes_total < 512);
}

#[test]
fn cross_calls_belong_to_the_explicit_child_window() {
    let env = TestEnvironment::no_cost();
    let leaf = env.register_mock(|c| {
        c.register_method(ContractEntrypointID::from_u64(1), || {});
    });
    leaf.deploy_contract_to(&LEAF);
    let result = env.transact(|tx| {
        tx.call(&ROOT)
            .run_continuable(|frame| {
                let child = frame.child_call(&CHILD).run(|| {
                    assert_eq!(
                        invoke(&LEAF, 1),
                        InvocationFrameStatusCode::Return
                    );
                });
                child.assert_success();
                assert_eq!(child.cross_call_alloc_records().len(), 1);
                assert_eq!(
                    child.cross_call_alloc_records()[0].stats,
                    AllocStats::default()
                );
                FrameAction::Close
            })
            .finish()
    });
    result.assert_success();
    assert_eq!(result.frame().alloc_stats(), Some(AllocStats::default()));
    assert!(result.frame().cross_call_alloc_records().is_empty());
}

#[test]
fn failed_cross_call_is_reported_without_leaking_to_the_next_transaction() {
    let env = TestEnvironment::no_cost();
    let child = env.register_mock(|c| {
        c.register_method(ContractEntrypointID::from_u64(1), || {
            allocate(1024, 1);
            panic!("intentional contract failure");
        });
    });
    child.deploy_contract_to(&CHILD);
    let result = env.transact(|tx| {
        tx.call(&ROOT).run(|| {
            assert_eq!(invoke(&CHILD, 1), InvocationFrameStatusCode::Fault);
        })
    });
    result.assert_success();
    let records = result.frame().cross_call_alloc_records();
    assert_eq!(records.len(), 1);
    assert!(records[0].stats.bytes_total >= 1024);
    let next = env.transact(|tx| {
        tx.call(&ROOT)
            .run_continuable(|_| FrameAction::Close)
            .finish()
    });
    next.assert_success();
    assert_eq!(next.frame().alloc_stats(), Some(AllocStats::default()));
    assert!(next.frame().cross_call_alloc_records().is_empty());
}

#[test]
fn independent_environments_do_not_steal_records_between_continuations() {
    let outer = TestEnvironment::no_cost();
    let inner = TestEnvironment::no_cost();
    for env in [&outer, &inner] {
        env.register_mock(|c| {
            c.register_method(ContractEntrypointID::from_u64(1), || {
                allocate(64, 1)
            });
        })
        .deploy_contract_to(&CHILD);
    }
    let result = outer.transact(|tx| {
        let mut pending = tx.call(&ROOT).run_continuable(|_| {
            assert_eq!(invoke(&CHILD, 1), InvocationFrameStatusCode::Return);
            FrameAction::Continue
        });
        let other = inner.transact(|tx| {
            tx.call(&ROOT).run(|| {
                assert_eq!(
                    invoke(&CHILD, 1),
                    InvocationFrameStatusCode::Return
                );
            })
        });
        other.assert_success();
        assert_eq!(other.frame().cross_call_alloc_records().len(), 1);
        pending
            .run_continuable(|_| {
                assert_eq!(
                    invoke(&CHILD, 1),
                    InvocationFrameStatusCode::Return
                );
                FrameAction::Close
            })
            .finish()
    });
    result.assert_success();
    assert_eq!(result.frame().cross_call_alloc_records().len(), 2);
}
