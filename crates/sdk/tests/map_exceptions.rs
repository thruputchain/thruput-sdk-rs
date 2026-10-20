#![allow(unexpected_cfgs)]

use thruput_sdk::call::RawCallError;
use thruput_sdk::*;

#[contract_exception]
pub enum ExampleError {
    Denied = 1,
}

pub struct Example;

#[contract]
impl Example {
    pub fn reply(mode: u32) -> u32 {
        match mode {
            0 => 42,
            1 => throw(ExampleError::Denied),
            2 => throw(LibraryExn::NonNegativeIntUnderflow),
            3 => panic!("test fault"),
            4 => contract_abort(&[0xff]),
            5 => internal::contract_return(&[0xff], &[]),
            _ => 0,
        }
    }

    pub fn read_gas() -> u64 {
        stats::gas_remaining()
    }

    pub fn write() {
        let mut slot: storage::Value<u32> = storage::Keypath::default().into();
        slot.set(&1);
    }
}

const TARGET: ContractAddress = ContractAddress::new([0x42; 32]);
const CALLER: ContractAddress = ContractAddress::new([0x41; 32]);

fn setup() -> TestEnvironmentHandle {
    let env = TestEnvironment::no_cost();
    env.register_mock(Example::__mock)
        .deploy_contract_to(&TARGET);
    env
}

#[test]
fn maps_only_parsed_exceptions_and_accepts_fn_once() {
    let env = setup();
    env.transact(|tx| {
        tx.call(&CALLER).run(|| {
            let client = ExampleClient::new(&TARGET);
            // Moving a non-Copy value out proves the handler may be FnOnce.
            let label = String::from("denied");
            let result = client
                .map_exceptions(move |exn| {
                    assert!(matches!(
                        exn.try_as::<ExampleError>(),
                        Some(ExampleError::Denied)
                    ));
                    label
                })
                .try_reply(1);
            assert_eq!(
                result,
                Err(RawCallError::Exception(String::from("denied")))
            );

            let result = client
                .map_exceptions(|exn| {
                    match_exn!(exn, {
                        LibraryExn::NonNegativeIntUnderflow => 7u8,
                        _ => 0,
                    })
                })
                .try_reply(2);
            assert_eq!(result, Err(RawCallError::Exception(7)));

            for mode in [0, 3, 4, 5] {
                let result = client
                    .map_exceptions(|_| -> () {
                        panic!("must not map this outcome")
                    })
                    .try_reply(mode);
                match mode {
                    0 => assert_eq!(result, Ok(42)),
                    3 => {
                        assert!(matches!(result, Err(RawCallError::Fault(_))))
                    }
                    4 => assert!(matches!(
                        result,
                        Err(RawCallError::UnparseableAbort(_))
                    )),
                    5 => assert!(matches!(
                        result,
                        Err(RawCallError::UnparseableReturn(_))
                    )),
                    _ => unreachable!(),
                }
            }
            let absent = ExampleClient::new(&ContractAddress::new([0x43; 32]));
            assert_eq!(
                absent
                    .map_exceptions(|_| -> () { panic!("callee never ran") })
                    .try_reply(0),
                Err(RawCallError::Invalid),
            );
            // The original client retains its ordinary owned-error path.
            assert!(matches!(
                client.try_reply(1),
                Err(CallError::Exception(_))
            ));
        })
    })
    .assert_success();
}

#[test]
fn preserves_invocation_flags_and_allows_nested_calls() {
    let env = setup();
    env.transact(|tx| {
        tx.call(&CALLER).run(|| {
            let client = ExampleClient::new(&TARGET);
            let limited = client
                .with_gas_limit(1)
                .map_exceptions(|_| -> () {
                    panic!("gas fault is not an exception")
                })
                .try_read_gas();
            assert!(matches!(limited, Err(RawCallError::Fault(_))));
            assert!(client.read_gas() > 1);
            let ordinary = client.with_read_only().try_write();
            assert!(ordinary.is_err());
            let mapped = client
                .with_read_only()
                .map_exceptions(|exn| exn.to_owned())
                .try_write();
            assert_eq!(mapped, ordinary);
            client.write();

            let mapped = client
                .map_exceptions(|exn| {
                    assert!(exn.try_as::<ExampleError>().is_some());
                    assert_eq!(client.reply(0), 42);
                    // Nested scratch use must not invalidate the borrowed bytes.
                    assert!(exn.try_as::<ExampleError>().is_some());
                    9u8
                })
                .try_reply(1);
            assert_eq!(mapped, Err(RawCallError::Exception(9)));
        })
    })
    .assert_success();
}

#[test]
fn borrowed_rethrow_preserves_abort_bytes() {
    let env = setup();
    let original = env.transact(|tx| {
        tx.call(&CALLER).run(|| {
            ExampleClient::new(&TARGET).reply(1);
        })
    });
    let mapped = env.transact(|tx| {
        tx.call(&CALLER).run(|| {
            let _: Result<u32, RawCallError<()>> = ExampleClient::new(&TARGET)
                .map_exceptions(|exn| exn.re_throw())
                .try_reply(1);
        })
    });
    original.assert_abort();
    mapped.assert_abort();
    assert_eq!(
        original.frame().abort_buffer(),
        mapped.frame().abort_buffer()
    );
}
