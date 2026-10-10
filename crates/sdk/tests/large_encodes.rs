//! The encode side of the 500-byte buffer caps: a contract's own returns
//! and aborts, and the calldata it sends to other contracts. With `alloc`,
//! all three encode through one reused per-instance scratch buffer.

use sdk::contract::ContractEntrypointID;
use sdk::{ContractAddress, TestEnvironment, TestEnvironmentHandle};

const CHILD_ADDR: ContractAddress = ContractAddress::new([0x52; 32]);
const PARENT_ADDR: ContractAddress = ContractAddress::new([0x51; 32]);

fn entrypoint() -> ContractEntrypointID {
    ContractEntrypointID::from_u64(0x99AA_BBCC_DDEE_FF00)
}

#[test]
fn self_return_larger_than_500_bytes_encodes() {
    let payload: Vec<u8> = (0..600u32).map(|i| (i % 251) as u8).collect();
    let expected = sdk::xdr::cxs::encode(&payload).expect("encode failed");

    let env = TestEnvironment::no_cost();
    let result = env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(move || -> () {
            sdk::internal::contract_return_encoded(&payload);
        })
    });
    result.assert_success();
    assert_eq!(result.frame().return_buffer(), expected.as_slice());
}

#[test]
fn abort_larger_than_500_bytes_encodes() {
    let payload: Vec<u8> = (0..600u32).map(|i| (i % 249) as u8).collect();
    let expected = sdk::xdr::cxs::encode(&payload).expect("encode failed");

    let env = TestEnvironment::no_cost();
    let result = env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(move || -> () {
            sdk::contract_abort_encoded(&payload);
        })
    });
    assert_eq!(result.frame().abort_buffer(), expected.as_slice());
}

#[test]
fn outgoing_calldata_larger_than_500_bytes_encodes() {
    let payload: Vec<u8> = (0..700u32).map(|i| (i % 247) as u8).collect();
    let args = (payload,);
    let expected_calldata_len =
        sdk::xdr::cxs::encode(&args).expect("encode failed").len() as u64;

    // The child reports how many calldata bytes it received.
    let env: TestEnvironmentHandle = TestEnvironment::no_cost();
    let mock = env.register_mock(|c| {
        c.register_method(entrypoint(), || {
            let mut calldata = sdk::internal::new_scratch();
            sdk::contract_op::fetch_calldata_and_auth_tokens(&mut calldata);
            let calldata_len = calldata.len();
            let encoded = sdk::xdr::cxs::encode(&(calldata_len as u64))
                .expect("encode failed");
            sdk::sys::raw_Return(&encoded, false, &[]);
        });
    });
    mock.deploy_contract_to(&CHILD_ADDR);

    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(move || {
            // Call twice: the second call exercises scratch reuse after
            // the buffer served both encode and receive duty.
            for _ in 0..2 {
                let received_len: u64 = sdk::call::try_call_raw_with::<
                    _,
                    _,
                    sdk::exn::RawException,
                >(
                    &CHILD_ADDR,
                    entrypoint(),
                    u64::MAX,
                    &args,
                    0,
                    |exn| exn.to_owned(),
                )
                .expect("call failed");
                assert_eq!(received_len, expected_calldata_len);
            }
        })
    })
    .assert_success();
}
