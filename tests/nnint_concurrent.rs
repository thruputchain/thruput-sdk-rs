use sdk::*;
use sdk::sys_exn::nnint::*;

const ADDR: ContractAddress = ContractAddress::new([1u8; 32]);
const KEY: StorageKey = StorageKey::new([2u8; 32]);

#[test]
fn concurrent_set() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| non_negative_int_set(&KEY, 27))
    }).assert_success();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| non_negative_int_set(&KEY, 27))
    }).assert_success();
}

#[test]
fn conflicting_set() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| non_negative_int_set(&KEY, 27))
    }).assert_success();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| non_negative_int_set(&KEY, 4))
    }).assert_commit_failed_nondeterministically();
}

#[test]
fn concurrent_inc() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| non_negative_int_set(&KEY, 27))
    }).assert_success();

    env.advance_block();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| non_negative_int_inc(&KEY, -7))
    }).assert_success();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| non_negative_int_inc(&KEY, -20))
    }).assert_success();
}

#[test]
fn conflicting_inc() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| non_negative_int_set(&KEY, 27))
    }).assert_success();

    env.advance_block();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| non_negative_int_inc(&KEY, -7))
    }).assert_success();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| non_negative_int_inc(&KEY, -21))
    }).assert_commit_failed_nondeterministically();
}

#[test]
fn concurrent_set_and_inc() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| {
                non_negative_int_set(&KEY, 27);
                non_negative_int_inc(&KEY, -7);
            })
    }).assert_success();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| {
                non_negative_int_set(&KEY, 27);
                non_negative_int_inc(&KEY, -10);
            })
    }).assert_success();
}

#[test]
fn conflicting_set_and_inc() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| {
                non_negative_int_set(&KEY, 27);
                non_negative_int_inc(&KEY, -7);
            })
    }).assert_success();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| {
                non_negative_int_set(&KEY, 27);
                non_negative_int_inc(&KEY, -21);
            })
    }).assert_commit_failed_nondeterministically();
}
