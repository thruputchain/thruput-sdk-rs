use sdk::*;

const ADDR: ContractAddress = ContractAddress::new([1u8; 32]);
const KEY: StorageKey = static_key!("my rcq");

struct Buffer<const N: usize = 4>([u8; N]);

impl<const N: usize> AsRef<[u8]> for Buffer<N> {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<const N: usize> From<[u8; N]> for Buffer<N> {
    fn from(array: [u8; N]) -> Self {
        Self(array)
    }
}

const ENTRY_0: Buffer = Buffer([0; 4]);
const ENTRY_1: Buffer = Buffer([1; 4]);

#[test]
fn concurrent_insert() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| RCQ::new(KEY).insert(&ENTRY_0))
    }).assert_success();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| RCQ::new(KEY).insert(&ENTRY_0))
    }).assert_success();
}

#[test]
fn insert_then_consume() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| RCQ::new(KEY).insert(&ENTRY_0))
    }).assert_success();

    env.advance_block();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| {
                let _: Buffer = RCQ::new(KEY).consume();
            })
    }).assert_success();
}

#[test]
fn concurrent_insert_consume() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| RCQ::new(KEY).insert(&ENTRY_0))
    }).assert_success();

    env.transact(|tx| {
        tx.call(&ADDR)
            .run(|| {
                let _: Buffer = RCQ::new(KEY).consume();
            })
    }).assert_success();
}

#[test]
fn failing_consume_insert() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| tx.call(&ADDR).run(|| {
        let _: Buffer = RCQ::new(KEY).consume();
    })).assert_abort();

    env.transact(|tx| tx.call(&ADDR).run(|| {
        RCQ::new(KEY).insert(&ENTRY_0);
    })).assert_success();
}

#[test]
fn concurrent_consume() {
    let env = TestEnvironment::no_cost();

    env.transact(|tx| tx.call(&ADDR).run(|| {
        let mut rcq = RCQ::new(KEY);

        rcq.insert(&ENTRY_0);
        rcq.insert(&ENTRY_1);
    })).assert_success();

    env.advance_block();

    env.transact(|tx| tx.call(&ADDR).run(|| {
        let _: Buffer = RCQ::new(KEY).consume();
    })).assert_success();

    // XXX: Why does this fail?
    env.transact(|tx| tx.call(&ADDR).run(|| {
        let _: Buffer = RCQ::new(KEY).consume();
    })).assert_commit_failed_nondeterministically();
}
