//! Test "storage" datatypes: keypaths, handles, vectors and maps

use sdk::storage::*;
use sdk::*;
use std::panic::UnwindSafe;
const ADDR: ContractAddress = ContractAddress::new([1u8; 32]);

#[test]
fn test_rcq_raw() {
    let env = TestEnvironment::no_cost();

    run_rcq(&env, |mut rcq: RCQ<()>| {
        rcq.insert_raw(&[1; 1]);
        rcq.insert_raw(&[2; 1]);
        rcq.insert_raw(&[3; 1]);
        rcq.insert_raw(&[4; 1]);
        rcq.insert_raw(&[5; 1]);

        assert_eq!(rcq.consume_sized(), [1]);
        assert_eq!(rcq.peek_sized(), [2]);
        assert_eq!(rcq.peek_sized(), [2]);
        assert_eq!(rcq.consume_sized(), [2]);
        assert_eq!(rcq.consume_sized(), [3]);
        assert_eq!(rcq.peek_raw().as_ref(), &[4]);
        assert_eq!(rcq.peek_raw().as_ref(), &[4]);
        assert_eq!(rcq.consume_raw().as_ref(), &[4]);
        assert_eq!(rcq.try_peek_raw().map(|b| Vec::from(b)), Ok(vec![5]));
        assert_eq!(rcq.try_consume_raw().map(|b| Vec::from(b)), Ok(vec![5]));
        // Empty queue: the try_ forms report a storage-missing error
        // instead of aborting.
        assert!(rcq.try_peek_raw().unwrap_err().is_storage_missing());
        assert!(rcq.try_consume_raw().unwrap_err().is_storage_missing());
        assert!(rcq.try_peek_sized::<1>().unwrap_err().is_storage_missing());
        assert!(
            rcq.try_consume_sized::<1>()
                .unwrap_err()
                .is_storage_missing()
        );
    })
    .assert_success();
}

// A wrong-size entry is reported as RetrocausalQueueEntryWrongSize by the sized try_
// forms — and stays in the queue (checked by peeking before consuming).
#[test]
fn test_rcq_sized_wrong_size() {
    let env = TestEnvironment::no_cost();

    run_rcq(&env, |mut rcq: RCQ<()>| {
        rcq.insert_raw(&[7; 3]);

        assert_eq!(
            rcq.try_consume_sized::<2>(),
            Err(LibraryExn::RetrocausalQueueEntryWrongSize {
                expected: 2,
                found: 3
            })
        );
        assert_eq!(
            rcq.try_peek_sized::<2>(),
            Err(LibraryExn::RetrocausalQueueEntryWrongSize {
                expected: 2,
                found: 3
            })
        );

        // The mis-sized entry was not consumed by the failed reads.
        assert_eq!(rcq.consume_sized(), [7; 3]);
    })
    .assert_success();
}

#[derive(Debug, PartialEq, ReadXDR, WriteXDR)]
struct Foo(u32);

#[test]
fn test_rcq() {
    let env = TestEnvironment::no_cost();

    run_rcq(&env, |mut rcq| {
        rcq.insert(&Foo(1));
        rcq.insert(&Foo(2));
        rcq.insert(&Foo(3));

        assert_eq!(rcq.consume(), Foo(1));
        assert_eq!(rcq.peek(), Foo(2));
        assert_eq!(rcq.peek(), Foo(2));
        assert_eq!(rcq.consume(), Foo(2));
        assert_eq!(rcq.consume(), Foo(3));
        assert!(rcq.try_peek().unwrap_err().is_storage_missing());
        assert!(rcq.try_consume().unwrap_err().is_storage_missing());
        assert_eq!(rcq.try_peek().none_if_missing(), None);
        assert_eq!(rcq.try_consume().none_if_missing(), None);
    })
    .assert_success();
}

// Helpers

fn run_rcq<T>(
    env: &TestEnvironmentHandle,
    f: impl FnOnce(RCQ<T>) + UnwindSafe,
) -> TransactResult {
    run(env, || {
        let keypath = Keypath::default();
        let rcq: RCQ<T> = keypath.into();
        f(rcq)
    })
}

fn run(
    env: &TestEnvironmentHandle,
    f: impl FnOnce() + UnwindSafe,
) -> TransactResult {
    env.transact(|tx| tx.call(&ADDR).run(f))
}

#[allow(dead_code)]
fn tx_result_try_as<E: Exn>(txr: &TransactResult) -> Option<E> {
    let caught =
        RawException::from_abort_buf(txr.frame().abort_buffer()).unwrap();
    caught.try_as::<E>()
}
