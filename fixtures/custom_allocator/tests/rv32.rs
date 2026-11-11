//! The rv32 artifact, with the reclaiming bump allocator from src/main.rs.
//!
//! Each workload is sized from the guest heap so that the runtime's default
//! bump allocator, which never reuses memory, would exhaust it.
//!
//! Requires the lowered `custom_allocator` artifact:
//! `cargo gnaw build custom_allocator` writes it.

mod common;
use common::*;
use custom_allocator::CustomAllocatorClient;
use thruput_sdk::*;

// Test working directory is the example contract directory.
const RV32_PATH: &str = "../../target/thruput/custom_allocator.rv32a";

static NONCE: SHA256Hash = sha256_static!("CUSTOM_ALLOCATOR_NONCE");

/// Deploys the artifact and runs `test` against its client.
fn with_client(
    test: impl FnOnce(&CustomAllocatorClient) + std::panic::UnwindSafe,
) {
    let contract = load_rv32_contract(RV32_PATH);
    let env = TestEnvironment::no_cost();
    let hash = env.setup_create_contract(&contract);
    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&NONCE, &hash);
            test(&CustomAllocatorClient::new(&addr));
        })
    })
    .assert_success();
}

#[test]
fn rv32_churn_reuses_freed_buffers() {
    with_client(|c| {
        let heap = c.heap_bytes();
        assert!(heap > 0, "the guest reports its heap size");
        // Eight half-heap buffers: four heaps' worth of allocation.
        let bytes = heap / 2;
        assert_eq!(c.churn(8, bytes), churn_sum(8, bytes));
    });
}

#[test]
fn rv32_grow_resizes_in_place() {
    with_client(|c| {
        // Growing one byte per realloc to half the heap. Copying on every
        // realloc would allocate about len^2 / 2 bytes in total.
        let len = c.heap_bytes() / 2;
        assert_eq!(c.grow(len), grow_sum(len));
    });
}

#[test]
fn rv32_grow_past_heap_fails() {
    with_client(|c| {
        let heap = c.heap_bytes();
        assert!(c.try_grow(heap + 1).is_err());
    });
}
