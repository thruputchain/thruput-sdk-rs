//! Allocation estimates through the generated client and native dispatcher.
#![allow(unexpected_cfgs)]

use thruput_sdk::*;

thruput_sdk::install_counting_allocator!();

#[thruput_sdk::storage]
struct AllocatingContract;

#[contract]
impl AllocatingContract {
    pub fn allocate(&self) -> u32 {
        std::hint::black_box(vec![0u8; 32768]);
        42
    }
}

#[test]
fn generated_client_reports_callee_allocations() {
    let env = TestEnvironment::no_cost();
    let address = ContractAddress::new([0x42; 32]);
    let mock = env.register_mock(AllocatingContract::__mock);
    mock.deploy_contract_to(&address);
    let result = env.transact(|tx| {
        tx.call(&ContractAddress::new([0x41; 32])).run(|| {
            assert_eq!(AllocatingContractClient::new(&address).allocate(), 42);
        })
    });
    result.assert_success();
    let records = result.frame().cross_call_alloc_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].contract, mock.as_contract());
    assert_eq!(
        records[0].entrypoint,
        AllocatingContractEntrypoints::allocate.into()
    );
    assert!(records[0].stats.bytes_total >= 32768);
    assert!(result.frame().alloc_stats().unwrap().bytes_total < 16384);
}
