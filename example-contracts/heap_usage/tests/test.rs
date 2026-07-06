//! Demonstrates per-frame allocation stats in native contract tests.
//!
//! Install the counting allocator once per test binary; each cross-call
//! frame (a `client.foo()` invocation) then logs a record of its own
//! allocations, exclusive of any nested calls — mirroring the real
//! system, where every frame is a separate guest instance with its own
//! bump-allocator heap.

sdk::install_counting_allocator!();

use heap_usage::HeapUsageClient;
use sdk::*;

static NONCE: SHA256Hash = sdk::sha256_static!("HEAP_USAGE_TEST_NONCE");

#[test]
fn cross_call_frame_reports_the_contract_vec() {
    // The contract allocates a Vec<u64> of COUNT elements: 8 * COUNT
    // bytes on the guest bump allocator.
    const COUNT: u32 = 4096;
    const VEC_BYTES: u64 = 8 * COUNT as u64;

    let env = sdk::TestEnvironment::no_cost();
    let test_contract = heap_usage::register_mock_contract(&env);
    let contract_hash = test_contract.as_hash().clone();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&NONCE, &contract_hash);
            let client = HeapUsageClient::new(&addr);

            // Discard records from setup on this thread.
            sdk::alloc_stats::drain_frame_records();

            let expected: u64 = (0..COUNT as u64).sum();
            assert_eq!(client.sum_range(COUNT), expected);

            let records = sdk::alloc_stats::drain_frame_records();
            assert_eq!(
                records.len(),
                1,
                "one cross-call, one record: {records:?}"
            );
            let stats = &records[0].stats;
            assert!(
                stats.bytes_total >= VEC_BYTES,
                "sum_range's Vec must be charged to its frame, got {stats:?}"
            );
            assert!(
                stats.heap_used >= stats.bytes_total,
                "bump-cursor consumption includes alignment padding, \
                 got {stats:?}"
            );
        })
    });
    result.assert_success();

    // The calling frame is not charged for the callee's Vec: in the
    // real system the callee runs in its own guest instance.
    let caller = result.frame().alloc_stats();
    assert!(
        caller.bytes_total < VEC_BYTES,
        "caller must exclude the callee's allocations, got {caller:?}"
    );
}
