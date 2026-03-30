use sdk::*;

const ADDR: ContractAddress = ContractAddress::new([1u8; 32]);
const KEY: StorageKey = static_key!("my rcq");

type Entry = [u8; 4];
const ENTRY_0: Entry = [0; 4];
const ENTRY_1: Entry = [1; 4];

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

#[test]
fn concurrent_consume() {
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

    // XXX: Why does this fail?
    env.transact(|tx| {
        tx.call(&ADDR).run(|| {
            let _: Entry = retrocausal_queue_consume_sized(&KEY);
        })
    })
    .assert_commit_failed_nondeterministically();
}
