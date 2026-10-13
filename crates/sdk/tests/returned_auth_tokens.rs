//! Auth tokens a child returns join the caller under consecutive new ids,
//! which `GetReturnedBuffer` reports as a `(start, count)` range.

use thruput_sdk::contract::ContractEntrypointID;
use thruput_sdk::mintable::AuthTokenPreimage;
use thruput_sdk::signer::Signed;
use thruput_sdk::{ContractAddress, TestEnvironment};

/// A child returns two distinct tokens. The caller already holds a token, so
/// the returned range starts above 0, and decoding from it must hand each
/// `Signed` return the token the child put in that position.
#[test]
fn returned_auth_tokens_arrive_as_a_range_in_child_order() {
    const CHILD_ADDR: ContractAddress = ContractAddress::new([0x71; 32]);
    const PARENT_ADDR: ContractAddress = ContractAddress::new([0x72; 32]);
    let entrypoint = ContractEntrypointID::from_u64(0x0CA1_1DA7_A000_0003);

    let mint = |consumer: ContractAddress, data: u64| {
        thruput_sdk::signer::mint_auth_token::<u64>(
            thruput_sdk::block::get_block_height(),
            AuthTokenPreimage { consumer, data },
        )
    };

    let env = TestEnvironment::no_cost();
    let mock = env.register_mock(|c| {
        c.register_method(entrypoint, move || {
            let returned = (mint(PARENT_ADDR, 11), mint(PARENT_ADDR, 22));
            thruput_sdk::internal::contract_return_encoded(&returned)
        });
    });
    mock.deploy_contract_to(&CHILD_ADDR);

    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(move || {
            // Takes the caller's id 0, so the returned tokens take 1 and 2.
            let held = mint(PARENT_ADDR, 7);

            let (first, second): (Signed<u64>, Signed<u64>) =
                thruput_sdk::call::try_call_raw_with(
                    &CHILD_ADDR,
                    entrypoint,
                    u64::MAX,
                    &(),
                    0,
                    |exn| panic!("child aborted: {exn:?}"),
                )
                .expect("call returning two tokens must succeed");
            assert_eq!((first.handle(), second.handle()), (1, 2));
            assert_eq!(first.consume(), (11, CHILD_ADDR));
            assert_eq!(second.consume(), (22, CHILD_ADDR));
            assert_eq!(held.consume(), (7, PARENT_ADDR));
        })
    })
    .assert_success();
}
