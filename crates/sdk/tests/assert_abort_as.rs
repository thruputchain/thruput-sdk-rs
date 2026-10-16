//! `assert_abort_as`: typed decoding of a thrown contract exception from a
//! mock transaction, going through the real `[selector][payload]` wire format.

#![allow(unexpected_cfgs)]

use thruput_sdk::*;

const TEST_ADDR: ContractAddress = ContractAddress::new([1u8; 32]);

#[contract_exception]
pub enum ThrownError {
    Nope = 1,
    WithData(u32) = 2,
}

#[contract_exception]
pub enum OtherError {
    Nope = 1,
}

fn throwing(f: impl FnOnce() + std::panic::UnwindSafe) -> TransactResult {
    let env = TestEnvironment::no_cost();
    env.transact(|tx| tx.call(&TEST_ADDR).run(f))
}

#[test]
fn assert_abort_as_decodes_a_real_thrown_exception() {
    let result = throwing(|| throw(ThrownError::WithData(7)));
    let e: ThrownError = result.assert_abort_as();
    assert_eq!(e, ThrownError::WithData(7));
}

#[test]
fn assert_abort_as_works_on_the_frame_too() {
    let result = throwing(|| throw(ThrownError::Nope));
    assert_eq!(
        result.frame().assert_abort_as::<ThrownError>(),
        ThrownError::Nope
    );
}

#[test]
#[should_panic(expected = "expected exception")]
fn assert_abort_as_rejects_a_different_exception_type_with_same_payload() {
    // Same payload bytes as ThrownError::Nope; only the selector differs.
    let result = throwing(|| throw(OtherError::Nope));
    let _: ThrownError = result.assert_abort_as();
}

#[test]
#[should_panic(expected = "expected abort")]
fn assert_abort_as_panics_when_the_frame_returned() {
    let result = throwing(|| {});
    let _: ThrownError = result.assert_abort_as();
}

#[test]
fn or_throw_on_option() {
    assert_eq!(Some(3u32).or_throw(ThrownError::Nope), 3);
    let result = throwing(|| {
        let _: u32 = None.or_throw(ThrownError::Nope);
    });
    assert_eq!(result.assert_abort_as::<ThrownError>(), ThrownError::Nope);
}

/// A payload that is everything `Exn` needs and nothing more: no `Clone`.
/// `#[contract_exception]` must not derive `Clone` on the carrying type,
/// or this stops compiling.
#[contract_type]
#[derive(Debug, PartialEq)]
pub struct NonClonePayload {
    pub code: u32,
    pub detail: u64,
}

#[contract_exception]
pub enum EnumCarryingNonClone {
    Detailed(NonClonePayload) = 1,
}

#[contract_exception]
pub struct StructCarryingNonClone {
    pub payload: NonClonePayload,
}

#[test]
fn enum_exception_with_non_clone_payload_round_trips() {
    let result = throwing(|| {
        throw(EnumCarryingNonClone::Detailed(NonClonePayload {
            code: 3,
            detail: 0xDEAD_BEEF,
        }))
    });
    assert_eq!(
        result.assert_abort_as::<EnumCarryingNonClone>(),
        EnumCarryingNonClone::Detailed(NonClonePayload {
            code: 3,
            detail: 0xDEAD_BEEF,
        })
    );
}

#[test]
fn struct_exception_with_non_clone_payload_round_trips() {
    let result = throwing(|| {
        throw(StructCarryingNonClone {
            payload: NonClonePayload { code: 9, detail: 1 },
        })
    });
    assert_eq!(
        result.assert_abort_as::<StructCarryingNonClone>(),
        StructCarryingNonClone {
            payload: NonClonePayload { code: 9, detail: 1 },
        }
    );
}
