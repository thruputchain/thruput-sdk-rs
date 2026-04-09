use sdk::*;

const ADDR: ContractAddress = ContractAddress::new([1u8; 32]);
const KEY: StorageKey = static_key!("my rcq");

type Entry = [u8; 4];
const ENTRY_0: Entry = [0; 4];
const ENTRY_1: Entry = [1; 4];

// Two transactions inserting in the same block: Ok
#[test]
fn concurrent_insert() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| retrocausal_queue_insert(&KEY, &ENTRY_0))
    })
    .assert_success();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| retrocausal_queue_insert(&KEY, &ENTRY_0))
    })
    .assert_success();
}

// Insert in one block, consume in the next: Ok
#[test]
fn insert_then_consume() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| retrocausal_queue_insert(&KEY, &ENTRY_0))
    })
    .assert_success();

    env.advance_block();

    env.transact(|tx| {
        tx.call(&ADDR).run(|| {
            let _: Entry = retrocausal_queue_consume_sized(&KEY);
        })
    })
    .assert_success();
}

// Insert and consume in the same block: Happens to succeed because of lucky
// submission ordering.
#[test]
fn concurrent_insert_consume() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| retrocausal_queue_insert(&KEY, &ENTRY_0))
    })
    .assert_success();

    env.transact(|tx| {
        tx.call(&ADDR).run(|| {
            let _: Entry = retrocausal_queue_consume_sized(&KEY);
        })
    })
    .assert_success();
}

// Consume and insert in the same block: Happens to fail due to unlucky
// submission ordering.
#[test]
fn failing_consume_insert() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| {
        tx.call(&ADDR).run(|| {
            let _: Entry = retrocausal_queue_consume_sized(&KEY);
        })
    })
    .assert_abort();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| retrocausal_queue_insert(&KEY, &ENTRY_0))
    })
    .assert_success();
}

// Insert in one block, then two transactions in the next block, both
// consuming: Fails because `consume` doesn't commute.
#[test]
fn concurrent_consume_fails() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| {
        tx.call(&ADDR).run(|| {
            retrocausal_queue_insert(&KEY, &ENTRY_0);
            retrocausal_queue_insert(&KEY, &ENTRY_1);
        })
    })
    .assert_success();

    env.advance_block();

    env.transact(|tx| {
        tx.call(&ADDR).run(|| {
            let _: Entry = retrocausal_queue_consume_sized(&KEY);
        })
    })
    .assert_success();

    // Only one transaction per block can call `consume` on a particular RCQ
    env.transact(|tx| {
        tx.call(&ADDR).run(|| {
            let _: Entry = retrocausal_queue_consume_sized(&KEY);
        })
    })
    .assert_commit_failed_nondeterministically();
}
