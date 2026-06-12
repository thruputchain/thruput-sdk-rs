//! Calldata-fetch semantics.
//!
//! With the `alloc` feature (always on under `mock`), calldata buffers are
//! sized exactly via `GetCalldataLen`, so calldata larger than the legacy
//! 500-byte stack buffer must be fetched in full.

use sdk::{BytesM, ContractAddress, Named, TestEnvironment};

fn fetch_roundtrips(payload: Vec<u8>) {
    let env = TestEnvironment::no_cost();
    let expected = payload.clone();
    env.transact(|tx| {
        let addr = ContractAddress::named("calldata_roundtrip");
        tx.call(&addr)
            .with_calldata(BytesM::try_from(payload.clone()).unwrap())
            .run(move || {
                let (calldata, auth_ids) =
                    sdk::contract_op::get_calldata_with_auth_tokens();
                assert_eq!(calldata.as_slice(), expected.as_slice());
                assert!(auth_ids.is_empty());
            })
    })
    .assert_success();
}

#[test]
fn small_calldata_roundtrips() {
    fetch_roundtrips((0..100u32).map(|i| i as u8).collect());
}

#[test]
fn empty_calldata_roundtrips() {
    fetch_roundtrips(Vec::new());
}

#[test]
fn calldata_larger_than_the_legacy_stack_buffer_roundtrips() {
    // 2000 bytes: four times the old CALLDATA_MAX of 500.
    fetch_roundtrips((0..2000u32).map(|i| (i % 251) as u8).collect());
}
