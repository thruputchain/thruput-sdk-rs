//! End-to-end `sdk::U256` and `sdk::I256` coverage in the native harness.
//!
//! The root-exported 256-bit integers are exercised the two ways a contract
//! moves one: through a contract entrypoint (calldata argument + typed
//! return, via `try_call_raw_with`) and through `storage::Value<_>`
//! (persisted across a block boundary and read back from a fresh handle).
//!
//! These tests run the contract side as a host closure. The same paths
//! through a real compiled artifact are covered by
//! `example-contracts/u256_roundtrip/tests/rv32.rs`.

use sdk::contract::ContractEntrypointID;
use sdk::storage::{Keypath, Value};
use sdk::*;

const CHILD_ADDR: ContractAddress = ContractAddress::new([0x51; 32]);
const PARENT_ADDR: ContractAddress = ContractAddress::new([0x52; 32]);

fn entrypoint() -> ContractEntrypointID {
    ContractEntrypointID::from_u64(0x0256_0256_0256_0256)
}

fn signed_entrypoint() -> ContractEntrypointID {
    ContractEntrypointID::from_u64(0x0257_0257_0257_0257)
}

/// A value whose limbs all differ and whose top bit is set, so any limb
/// reordering or truncation in the calldata/return/storage paths shows up.
fn test_value() -> U256 {
    U256::widening_mul(
        0xDEAD_BEEF_0011_2233_4455_6677_8899_AABB_u128,
        0x8000_0000_0000_0000_0000_0000_0000_0001_u128,
    )
}

/// The signed counterpart, and negative -- a codec that zero-extends instead
/// of carrying the two's complement bits loses exactly this.
fn signed_test_value() -> I256 {
    -I256::from(0x0DEA_DBEE_F001_1223_3445_5667_7889_9AAB_i128)
}

/// U256 through a contract entrypoint: the child decodes its calldata as a
/// U256, adds one, and returns the sum through the typed return ABI.
#[test]
fn u256_roundtrips_through_entrypoint_calldata_and_return() {
    let env = TestEnvironment::no_cost();
    let mock = env.register_mock(|c| {
        c.register_method(entrypoint(), || {
            let mut calldata = sdk::internal::new_scratch();
            sdk::contract_op::fetch_calldata_and_auth_tokens(&mut calldata);
            let arg: U256 = sdk::xdr::cxs::decode(calldata.as_slice())
                .expect("calldata must decode as U256");
            let out = arg.checked_add(U256::from(1)).unwrap();
            sdk::internal::contract_return_encoded(&out)
        });
    });
    mock.deploy_contract_to(&CHILD_ADDR);

    let arg = test_value();
    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(move || {
            let returned: U256 = sdk::call::try_call_raw_with(
                &CHILD_ADDR,
                entrypoint(),
                u64::MAX,
                &arg,
                0,
                |exn| panic!("child aborted: {exn:?}"),
            )
            .expect("typed U256 call must succeed");
            assert_eq!(returned, arg.checked_add(U256::from(1)).unwrap());
        })
    })
    .assert_success();
}

/// U256 through `storage::Value<U256>`: written in one transaction,
/// committed across a block boundary, and read back through a fresh storage
/// handle in the next.
#[test]
fn u256_roundtrips_through_storage_value() {
    let env = TestEnvironment::no_cost();
    let value = test_value();

    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(move || {
            let mut slot: Value<U256> = Keypath::default().into();
            slot.set(&value);
            // Read back within the same frame.
            assert_eq!(slot.get(), value);
        })
    })
    .assert_success();

    env.advance_block();

    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(move || {
            let slot: Value<U256> = Keypath::default().into();
            assert_eq!(slot.get(), value);
        })
    })
    .assert_success();
}

/// I256 through a contract entrypoint: the child decodes its calldata as an
/// I256, negates it, and returns the result through the typed return ABI.
#[test]
fn i256_roundtrips_through_entrypoint_calldata_and_return() {
    let env = TestEnvironment::no_cost();
    let mock = env.register_mock(|c| {
        c.register_method(signed_entrypoint(), || {
            let mut calldata = sdk::internal::new_scratch();
            sdk::contract_op::fetch_calldata_and_auth_tokens(&mut calldata);
            let arg: I256 = sdk::xdr::cxs::decode(calldata.as_slice())
                .expect("calldata must decode as I256");
            let out = arg.checked_neg().unwrap();
            sdk::internal::contract_return_encoded(&out)
        });
    });
    mock.deploy_contract_to(&CHILD_ADDR);

    let arg = signed_test_value();
    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(move || {
            let returned: I256 = sdk::call::try_call_raw_with(
                &CHILD_ADDR,
                signed_entrypoint(),
                u64::MAX,
                &arg,
                0,
                |exn| panic!("child aborted: {exn:?}"),
            )
            .expect("typed I256 call must succeed");
            assert!(arg.is_negative() && returned.is_positive());
            assert_eq!(returned, arg.checked_neg().unwrap());
        })
    })
    .assert_success();
}

/// I256 through `storage::Value<I256>`, across a block boundary.
#[test]
fn i256_roundtrips_through_storage_value() {
    let env = TestEnvironment::no_cost();
    let value = signed_test_value();

    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(move || {
            let mut slot: Value<I256> = Keypath::default().into();
            slot.set(&value);
            assert_eq!(slot.get(), value);
        })
    })
    .assert_success();

    env.advance_block();

    env.transact(|tx| {
        tx.call(&PARENT_ADDR).run(move || {
            let slot: Value<I256> = Keypath::default().into();
            assert_eq!(slot.get(), value);
            assert!(slot.get().is_negative());
        })
    })
    .assert_success();
}
