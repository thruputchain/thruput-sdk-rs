#![cfg_attr(not(feature = "dev"), no_std)]

extern crate alloc;
use alloc::vec::Vec;

use sdk::sys_result::rcq::*;
use sdk::*;

const KEY1: StorageKey = static_key!("queue key");
const KEY2: StorageKey = static_key!("queue key2");

#[contract]
fn test_read_self_item() {
    retrocausal_queue_insert(&KEY1, &[1]).unwrap();
    retrocausal_queue_insert(&KEY1, &[2]).unwrap();

    let item = retrocausal_queue_peek_sized::<1>(&KEY1).unwrap();
    assert_eq!(item[0], 1);

    let item = retrocausal_queue_consume_sized::<1>(&KEY1).unwrap();
    assert_eq!(item[0], 1);

    let item = retrocausal_queue_consume_sized::<1>(&KEY1).unwrap();
    assert_eq!(item[0], 2);

    if retrocausal_queue_peek_sized::<1>(&KEY1).is_ok() {
        panic!("!");
    }
}

#[contract]
fn make_several_inserts(inserts: Vec<u8>) {
    for i in inserts {
        retrocausal_queue_insert(&KEY1, &[i]).unwrap();
    }
}

#[contract]
fn expect_several_consumes(expected: Vec<u8>) {
    for i in expected {
        let item = retrocausal_queue_consume_sized::<1>(&KEY1).unwrap();
        assert_eq!(item[0], i);
    }
}

#[contract]
fn prep_for_test_read_weird_order() {
    retrocausal_queue_insert(&KEY1, &[1]).unwrap();
    retrocausal_queue_insert(&KEY1, &[2]).unwrap();
    retrocausal_queue_insert(&KEY1, &[3]).unwrap();
    retrocausal_queue_insert(&KEY2, &[4]).unwrap();
    retrocausal_queue_insert(&KEY2, &[5]).unwrap();
}

#[contract]
fn test_read_weird_order() {
    // Expected ndet proposal queue orders:
    // Key1: 1, 2, 3
    // Key2: 4, 5
    // Key2 inserts a 6

    let item = retrocausal_queue_consume_sized::<1>(&KEY1).unwrap();
    assert_eq!(item[0], 1);

    let item = retrocausal_queue_consume_sized::<1>(&KEY1).unwrap();
    assert_eq!(item[0], 2);

    let item = retrocausal_queue_peek_sized::<1>(&KEY2).unwrap();
    assert_eq!(item[0], 4);

    retrocausal_queue_insert(&KEY2, &[6]).unwrap();

    let item = retrocausal_queue_peek_sized::<1>(&KEY1).unwrap();
    assert_eq!(item[0], 3);

    let item = retrocausal_queue_consume_sized::<1>(&KEY1).unwrap();
    assert_eq!(item[0], 3);

    let item = retrocausal_queue_consume_sized::<1>(&KEY2).unwrap();
    assert_eq!(item[0], 4);

    let item = retrocausal_queue_consume_sized::<1>(&KEY2).unwrap();
    assert_eq!(item[0], 5);

    let item = retrocausal_queue_consume_sized::<1>(&KEY2).unwrap();
    assert_eq!(item[0], 6);
}
