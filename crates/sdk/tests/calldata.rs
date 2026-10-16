//! Calldata-fetch semantics.
//!
//! With the `alloc` feature (always on under `mock`), calldata buffers are
//! sized exactly via `GetCalldataLen`, so calldata larger than the legacy
//! 500-byte stack buffer must be fetched in full.
//!
//! The fetch also reports the number N of auth tokens forwarded with the
//! call, which are the callee's auth token ids 0..N-1 in the order the
//! caller listed them.

use thruput_sdk::contract::ContractEntrypointID;
use thruput_sdk::mintable::AuthTokenPreimage;
use thruput_sdk::signer::Signed;
use thruput_sdk::{BytesM, ContractAddress, Named, TestEnvironment};

fn fetch_roundtrips(payload: Vec<u8>) {
    let env = TestEnvironment::no_cost();
    let expected = payload.clone();
    env.transact(|tx| {
        let addr = ContractAddress::named("calldata_roundtrip");
        tx.call(&addr)
            .with_calldata(BytesM::try_from(payload.clone()).unwrap())
            .run(move || {
                let mut calldata = thruput_sdk::internal::new_scratch();
                let auth_token_count =
                    thruput_sdk::contract_op::fetch_calldata_and_auth_tokens(
                        &mut calldata,
                    );
                assert_eq!(calldata.as_slice(), expected.as_slice());
                assert_eq!(auth_token_count, 0);
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

/// Two tokens forwarded with one call arrive as a count of 2, and decoding
/// with handles 0..N, as the generated entrypoint wrappers do, hands each
/// `Signed` argument the token the caller put in that position.
#[test]
fn forwarded_auth_tokens_arrive_as_ids_0_to_n_in_caller_order() {
    const CHILD_ADDR: ContractAddress = ContractAddress::new([0x61; 32]);
    const PARENT_ADDR: ContractAddress = ContractAddress::new([0x62; 32]);
    let entrypoint = ContractEntrypointID::from_u64(0x0CA1_1DA7_A000_0002);

    let env = TestEnvironment::no_cost();
    let mock = env.register_mock(|c| {
        c.register_method(entrypoint, || {
            let mut calldata = thruput_sdk::internal::new_scratch();
            let auth_token_count =
                thruput_sdk::contract_op::fetch_calldata_and_auth_tokens(
                    &mut calldata,
                );
            assert_eq!(auth_token_count, 2);
            let (first, second): (Signed<u64>, Signed<u64>) =
                thruput_sdk::call::contract_call_decode(
                    calldata.as_slice(),
                    0..auth_token_count,
                )
                .expect("both Signed arguments must decode");
            let (first, _) = first.consume();
            let (second, _) = second.consume();
            thruput_sdk::internal::contract_return_encoded(&(first, second))
        });
    });
    mock.deploy_contract_to(&CHILD_ADDR);

    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(move || {
            let mint = |data: u64| {
                thruput_sdk::signer::mint_auth_token::<u64>(
                    thruput_sdk::block::get_block_height(),
                    AuthTokenPreimage {
                        consumer: CHILD_ADDR,
                        data,
                    },
                )
            };
            let args = (mint(11), mint(22));
            let returned: (u64, u64) = thruput_sdk::call::try_call_raw_with(
                &CHILD_ADDR,
                entrypoint,
                u64::MAX,
                &args,
                0,
                |exn| panic!("child aborted: {exn:?}"),
            )
            .expect("call with two forwarded tokens must succeed");
            assert_eq!(returned, (11, 22));
        })
    })
    .assert_success();
}
