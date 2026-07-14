//! Per-frame allocation stats through the SDK's real client-call path:
//! `try_call_raw_with` → `Invoke` syscall → hogd → trampoline → mock method.

sdk::install_counting_allocator!();

use sdk::contract::ContractEntrypointID;
use sdk::{ContractAddress, TestEnvironment};

const PARENT_ADDR: ContractAddress = ContractAddress::new([0x41; 32]);
const CHILD_ADDR: ContractAddress = ContractAddress::new([0x42; 32]);

#[test]
fn client_style_cross_call_produces_labeled_alloc_record() {
    const MOCK_ALLOC: usize = 32 * 1024;
    let entrypoint = ContractEntrypointID::from_u64(0x0102_0304_0506_0708);

    let env = TestEnvironment::no_cost();
    let mock = env.register_mock(|c| {
        c.register_method(entrypoint, || {
            let v = vec![0u8; MOCK_ALLOC];
            std::hint::black_box(&v);
        });
    });
    mock.deploy_contract_to(&CHILD_ADDR);

    let result = env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(|| {
            let () = sdk::call::try_call_raw_with(
                &CHILD_ADDR,
                entrypoint,
                u64::MAX,
                &(),
                0,
                |exn| exn.to_owned(),
            )
            .expect("cross-call failed");
        })
    });
    result.assert_success();

    let records = result.frame().cross_call_alloc_records();
    assert_eq!(
        records.len(),
        1,
        "one client-style cross-call must produce one record, got {records:?}"
    );
    assert_eq!(records[0].entrypoint, entrypoint);
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
