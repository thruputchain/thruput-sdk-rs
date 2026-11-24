//! `Rollover`: a cell that starts over when its period changes.
//!
//! The tests that matter put many transactions in the first block of a new
//! period, each executed before any commits. They must all commit, in any
//! order, and add up to the same value.

use core::num::NonZeroU64;
use std::time::Duration;
use thruput_sdk::exn::LibraryExn;
use thruput_sdk::storage::*;
use thruput_sdk::*;

const ADDR: ContractAddress = ContractAddress::new([7u8; 32]);

const SECOND: u64 = 1_000_000_000;
const HOUR: NonZeroU64 = NonZeroU64::new(3_600 * SECOND).unwrap();
// Windows are counted from here; the first test block is at their start.
const ORIGIN: u64 = 4_000_000_000 * SECOND;

// How many transactions share the first block of a period.
const CROWD: usize = 40;
const CROWD_SUM: i128 = (CROWD * (CROWD + 1) / 2) as i128;

fn cell<L: TagLayout>() -> Rollover<L> {
    Keypath::default().child(&1u32).into()
}

fn window() -> Period {
    Period::window(ORIGIN, HOUR)
}

// An environment whose current block is at the start of window 0, with
// blocks a second apart after it.
fn start() -> TestEnvironmentHandle {
    let env = TestEnvironment::no_cost();
    env.advance_block_at(ORIGIN.into());
    env.set_block_time_step(Some(Duration::from_secs(1)));
    env
}

fn next_window(env: &TestEnvironmentHandle, n: u64) {
    env.advance_block_at((ORIGIN + n * HOUR.get()).into());
}

// Run `f` as one committed transaction and return what it returned.
#[track_caller]
fn run<T>(
    env: &TestEnvironmentHandle,
    f: impl FnOnce() -> T + std::panic::UnwindSafe,
) -> T
where
    T: WriteXDR + for<'a> ReadXDR<'a>,
{
    let result = env.transact(|tx| tx.call(&ADDR).run(f));
    result.assert_success();
    result.assert_return_as::<T>()
}

// Execute `f` in the current block without committing it.
#[track_caller]
fn pending(
    env: &TestEnvironmentHandle,
    f: impl FnOnce() + std::panic::UnwindSafe,
) -> MockTransactionHandler {
    let mut tx = env.transaction().build();
    tx.call(&ADDR).run(f).assert_success();
    tx
}

fn conflict_codes(result: &TryCommitResult) -> Vec<DbConflictCode> {
    match result {
        TryCommitResult::TxNonDeterministicError(
            DbConflictResult::kDbDiagnosticAll { conflicts },
        ) => conflicts.iter().map(|c| c.code).collect(),
        TryCommitResult::TxNonDeterministicError(
            DbConflictResult::kDbDiagnosticFirst { first },
        ) => vec![first.code],
        other => panic!("expected a conflict, got {other:?}"),
    }
}

#[derive(Clone, Copy)]
enum Order {
    Forward,
    Backward,
}

// `setup` in window 0. Then, in the first block of window 2, CROWD
// transactions each run `touch(i)`, all executed before any commits, and
// commit in `order`. Returns what `read` finds in the next block.
fn crowd(
    order: Order,
    setup: fn(),
    touch: fn(usize),
    read: fn() -> i128,
) -> i128 {
    let env = start();
    run(&env, setup);
    next_window(&env, 2);

    let mut txs: Vec<_> = (0..CROWD)
        .map(|i| pending(&env, move || touch(i)))
        .collect();
    if let Order::Backward = order {
        txs.reverse();
    }
    for (n, tx) in txs.iter_mut().enumerate() {
        assert_eq!(tx.try_commit_transaction(), TryCommitResult::Ok, "{n}");
    }
    drop(txs);

    env.advance_block();
    run(&env, read)
}

// Both orders commit every transaction and leave the same value.
#[track_caller]
fn crowd_agrees(setup: fn(), touch: fn(usize), read: fn() -> i128) -> i128 {
    let forward = crowd(Order::Forward, setup, touch, read);
    let backward = crowd(Order::Backward, setup, touch, read);
    assert_eq!(forward, backward);
    forward
}

// --- A fixed window: the reset is a constant -------------------------------

mod tally {
    use super::*;

    // What window 0 leaves behind, which window 2 must not count.
    pub fn leftover<L: TagLayout>() {
        cell::<L>().enter(window(), 0).add(777);
    }

    pub fn count<L: TagLayout>(i: usize) {
        cell::<L>().enter(window(), 0).add(i as i128 + 1);
    }

    pub fn read<L: TagLayout>() -> i128 {
        cell::<L>().peek(window(), 0)
    }
}

#[test]
fn a_crowd_starts_a_window_together() {
    use tally::*;
    assert_eq!(
        crowd_agrees(leftover::<Split>, count::<Split>, read::<Split>),
        CROWD_SUM
    );
    assert_eq!(
        crowd_agrees(leftover::<Packed>, count::<Packed>, read::<Packed>),
        CROWD_SUM
    );
}

// The same, counting down from a limit: what a rate limit does, since the
// runtime holds a block's merged total at or above zero.
mod quota {
    use super::*;

    pub const LIMIT: i128 = 10_000;

    pub fn spend_most<L: TagLayout>() {
        cell::<L>().enter(window(), LIMIT).add(-9_999);
    }

    pub fn spend<L: TagLayout>(i: usize) {
        let mut cell = cell::<L>();
        let mut left = cell.enter(window(), LIMIT);
        assert!(left.get() >= i as i128 + 1);
        left.add(-(i as i128 + 1));
    }

    pub fn read<L: TagLayout>() -> i128 {
        cell::<L>().peek(window(), LIMIT)
    }
}

#[test]
fn a_crowd_spends_from_a_fresh_quota() {
    use quota::*;
    let left = LIMIT - CROWD_SUM;
    assert_eq!(
        crowd_agrees(spend_most::<Split>, spend::<Split>, read::<Split>),
        left
    );
    assert_eq!(
        crowd_agrees(spend_most::<Packed>, spend::<Packed>, read::<Packed>),
        left
    );
}

// --- A carry-over: the reset depends on what the last period left ----------

mod decay {
    use super::*;

    // Half of what was there, for each window that has passed.
    pub fn halved(lapsed: Lapsed) -> Option<i128> {
        let windows = lapsed.to - lapsed.from.unwrap_or(lapsed.to);
        Some(lapsed.value >> windows.min(127))
    }

    pub fn leftover<L: TagLayout>() {
        cell::<L>().enter_with(window(), halved).add(1_000);
    }

    pub fn count<L: TagLayout>(i: usize) {
        cell::<L>().enter_with(window(), halved).add(i as i128 + 1);
    }

    pub fn read<L: TagLayout>() -> i128 {
        cell::<L>().peek_with(window(), halved)
    }
}

#[test]
fn a_crowd_carries_the_last_window_over() {
    use decay::*;
    // Two windows on, a quarter of the 1,000 is left.
    let expected = 250 + CROWD_SUM;
    assert_eq!(
        crowd_agrees(leftover::<Split>, count::<Split>, read::<Split>),
        expected
    );
    assert_eq!(
        crowd_agrees(leftover::<Packed>, count::<Packed>, read::<Packed>),
        expected
    );
}

// --- Within one transaction ------------------------------------------------

fn second_touch_keeps_the_first<L: TagLayout>() {
    let env = start();
    run(&env, tally::leftover::<L>);
    next_window(&env, 1);

    let seen = run(&env, || {
        let mut cell = cell::<L>();
        cell.enter(window(), 0).add(5);
        // A new handle, as a second call in the transaction would make.
        let mut again = self::cell::<L>();
        let mut current = again.enter(window(), 0);
        current.add(7);
        current.get()
    });
    assert_eq!(seen, 12);

    env.advance_block();
    assert_eq!(run(&env, tally::read::<L>), 12);
}

#[test]
fn a_second_touch_in_a_transaction_does_not_reset_again() {
    second_touch_keeps_the_first::<Split>();
    second_touch_keeps_the_first::<Packed>();
}

fn rule_sees_the_snapshot<L: TagLayout>() {
    let env = start();
    run(&env, decay::leftover::<L>);
    next_window(&env, 1);

    let calls = run(&env, || {
        let mut calls = 0u32;
        for _ in 0..3 {
            cell::<L>()
                .enter_with(window(), |lapsed| {
                    calls += 1;
                    // The block's start, not this transaction's additions.
                    assert_eq!(lapsed.value, 1_000);
                    assert_eq!((lapsed.from, lapsed.to), (Some(0), 1));
                    decay::halved(lapsed)
                })
                .add(1);
        }
        calls
    });
    assert_eq!(calls, 1);

    env.advance_block();
    assert_eq!(run(&env, decay::read::<L>), 503);
}

#[test]
fn a_rule_runs_once_on_the_snapshot() {
    rule_sees_the_snapshot::<Split>();
    rule_sees_the_snapshot::<Packed>();
}

// A transaction whose period changes under it would reset twice and lose its
// own first addition. It is refused instead.
fn refuses_a_second_period<L: TagLayout>() {
    let env = start();
    let refused = run(&env, || {
        let mut cell = cell::<L>();
        cell.enter(Period::from_raw(5), 100).add(-60);
        let refused = cell.try_enter(Period::from_raw(6), 100).err();
        // Nothing was discarded.
        assert_eq!(cell.enter(Period::from_raw(5), 100).get(), 40);
        refused == Some(LibraryExn::RolloverAlreadyWritten)
    });
    assert!(refused);
}

#[test]
fn a_period_that_changes_within_a_transaction_is_refused() {
    refuses_a_second_period::<Split>();
    refuses_a_second_period::<Packed>();
}

// A rule that first says there is nothing to record, and later in the same
// transaction says there is, would discard what was added in between.
#[test]
fn a_rule_that_changes_its_mind_within_a_transaction_is_refused() {
    let env = start();
    run(&env, tally::leftover::<Split>);
    next_window(&env, 1);

    let refused = run(&env, || {
        let mut cell = cell::<Split>();
        cell.enter_with(window(), |_| None).add(1);
        // Asking again and hearing the same is fine.
        cell.enter_with(window(), |_| None).add(1);
        assert_eq!(cell.recorded(), (Some(0), 779));
        let refused = cell.try_enter_with(window(), |_| Some(0)).err();
        refused == Some(LibraryExn::RolloverAlreadyWritten)
    });
    assert!(refused);
}

#[test]
fn restart_does_not_discard_this_transactions_additions() {
    let env = start();
    let refused = run(&env, || {
        let mut cell = cell::<Split>();
        cell.restart(window(), 50);
        cell.enter(window(), 0).add(1);
        cell.try_restart(window(), 50)
            == Err(LibraryExn::RolloverAlreadyWritten)
    });
    assert!(refused);
    env.advance_block();
    assert_eq!(run(&env, tally::read::<Split>), 51);
}

// --- Readers ---------------------------------------------------------------

fn stale_read_is_empty<L: TagLayout>() {
    let env = start();
    run(&env, tally::leftover::<L>);
    env.advance_block();
    assert_eq!(run(&env, tally::read::<L>), 777);

    next_window(&env, 3);
    assert_eq!(run(&env, tally::read::<L>), 0);
    assert_eq!(run(&env, quota::read::<L>), quota::LIMIT);

    // The reads recorded nothing: storage still holds window 0's tally.
    env.advance_block();
    let (tag, value) = run(&env, || {
        let (tag, value) = cell::<L>().recorded();
        (tag.unwrap_or(u64::MAX), value)
    });
    assert_eq!((tag, value), (0, 777));
}

#[test]
fn a_stale_read_is_empty_and_writes_nothing() {
    stale_read_is_empty::<Split>();
    stale_read_is_empty::<Packed>();
}

#[test]
fn a_never_written_cell_reads_as_reset() {
    let env = start();
    next_window(&env, 4);
    let (split, packed) = run(&env, || {
        let split = cell::<Split>();
        let packed = cell::<Packed>();
        assert_eq!(split.recorded(), (None, 0));
        assert_eq!(packed.recorded(), (None, 0));
        (split.peek(window(), 9), packed.peek(window(), 9))
    });
    assert_eq!((split, packed), (9, 9));
}

// --- What the runtime refuses ----------------------------------------------

// The block's spends are held to what the quota had: the runtime admits them
// while they fit, whatever each saw when it executed.
#[test]
fn a_block_cannot_overspend_a_fresh_quota() {
    let env = start();
    next_window(&env, 1);
    let spend = || {
        let mut cell = cell::<Packed>();
        let mut left = cell.enter(window(), 100);
        assert_eq!(left.get(), 100);
        left.add(-40);
    };
    let mut txs: Vec<_> = (0..3).map(|_| pending(&env, spend)).collect();
    let results: Vec<_> = txs
        .iter_mut()
        .map(|tx| tx.try_commit_transaction())
        .collect();
    drop(txs);

    assert_eq!(results[..2], [TryCommitResult::Ok, TryCommitResult::Ok]);
    assert_eq!(
        conflict_codes(&results[2]),
        [DbConflictCode::BoundsExceeded]
    );
    env.advance_block();
    assert_eq!(run(&env, || cell::<Packed>().peek(window(), 100)), 20);
}

// The known hazard. A transaction that computes a different reset from the
// rest of the block (here, as if an earlier call in it had lowered the
// limit) cannot share the block with them. Whichever commits second loses.
fn different_resets_conflict<L: TagLayout>() {
    let env = start();
    next_window(&env, 1);
    let spend =
        |limit: i128| move || cell::<L>().enter(window(), limit).add(-1);

    let mut usual = pending(&env, spend(100));
    let mut odd = pending(&env, spend(50));
    let mut late = pending(&env, spend(100));
    assert_eq!(usual.try_commit_transaction(), TryCommitResult::Ok);
    assert_eq!(
        conflict_codes(&odd.try_commit_transaction()),
        [DbConflictCode::ValueMismatch]
    );
    assert_eq!(late.try_commit_transaction(), TryCommitResult::Ok);
    drop((usual, odd, late));

    env.advance_block();
    assert_eq!(run(&env, || cell::<L>().peek(window(), 100)), 98);
}

#[test]
fn transactions_that_compute_different_resets_conflict() {
    different_resets_conflict::<Split>();
    different_resets_conflict::<Packed>();
}

// --- A token bucket --------------------------------------------------------
//
// All of the bucket a contract would write. The period is the block's
// timepoint, and the rule credits what has flowed in since the last one.

mod bucket {
    use super::*;

    /// `burst` at most, refilled by `refill` units every `period` seconds.
    #[derive(Clone, Copy)]
    pub struct Rate {
        pub burst: i128,
        pub refill: u64,
        pub period: u64,
    }

    impl Rate {
        // The whole units that have flowed in from the Unix epoch to `t`.
        // Crediting differences of this loses no fraction of a unit, however
        // often it is done.
        fn flowed(&self, t: u64) -> i128 {
            let period = u128::from(self.period) * u128::from(SECOND);
            u256::mul_div_u128(self.refill.into(), t.into(), period) as i128
        }

        fn refilled(&self, lapsed: Lapsed) -> Option<i128> {
            // A bucket never filled stays empty.
            let credit = self.flowed(lapsed.to) - self.flowed(lapsed.from?);
            let room = self.burst.saturating_sub(lapsed.value);
            if credit >= room {
                Some(self.burst)
            } else if credit == 0 {
                // Nothing to record: leave the count where it was.
                None
            } else {
                Some(lapsed.value + credit)
            }
        }
    }

    pub struct Bucket(Rollover);

    impl Bucket {
        pub fn at(slot: u32) -> Self {
            Self(Keypath::default().child(&slot).into())
        }

        pub fn fill(&mut self, rate: Rate) {
            self.0.restart(Period::timepoint(), rate.burst);
        }

        pub fn available(&self, rate: Rate) -> i128 {
            self.0
                .peek_with(Period::timepoint(), |lapsed| rate.refilled(lapsed))
        }

        pub fn take(&mut self, rate: Rate, amount: i128) -> bool {
            let mut held = self.0.enter_with(Period::timepoint(), |lapsed| {
                rate.refilled(lapsed)
            });
            if held.get() < amount {
                return false;
            }
            held.add(-amount);
            true
        }
    }
}

use bucket::{Bucket, Rate};

// A unit a second, up to a hundred.
const PER_SECOND: Rate = Rate {
    burst: 100,
    refill: 60,
    period: 60,
};

// Seven units every three seconds: a unit does not arrive on the second.
const UNEVEN: Rate = Rate {
    burst: 1_000_000,
    refill: 7,
    period: 3,
};

fn at(env: &TestEnvironmentHandle, seconds: u64) {
    env.advance_block_at((ORIGIN + seconds * SECOND).into());
}

#[test]
fn a_bucket_spends_its_burst_then_what_flows_back() {
    let env = start();
    run(&env, || Bucket::at(1).fill(PER_SECOND));
    env.advance_block();

    assert!(run(&env, || Bucket::at(1).take(PER_SECOND, 100)));
    env.advance_block();
    // A second later, a unit.
    assert_eq!(run(&env, || Bucket::at(1).available(PER_SECOND)), 1);
    assert!(!run(&env, || Bucket::at(1).take(PER_SECOND, 2)));
    assert!(run(&env, || Bucket::at(1).take(PER_SECOND, 1)));

    // An hour later it is full, and no fuller.
    at(&env, 3_600);
    assert_eq!(run(&env, || Bucket::at(1).available(PER_SECOND)), 100);

    // A bucket nobody filled holds nothing, however long it waits.
    assert_eq!(run(&env, || Bucket::at(2).available(PER_SECOND)), 0);
    assert!(!run(&env, || Bucket::at(2).take(PER_SECOND, 1)));
}

// Empty the bucket at second 1, touch it at each of `touches`, and return
// what it holds at second 100.
fn refilled_after(touches: &[u64]) -> i128 {
    let env = start();
    run(&env, || Bucket::at(1).fill(UNEVEN));
    at(&env, 1);
    assert!(run(&env, || Bucket::at(1).take(UNEVEN, UNEVEN.burst)));
    for &second in touches {
        at(&env, second);
        assert!(run(&env, || Bucket::at(1).take(UNEVEN, 0)));
    }
    at(&env, 100);
    run(&env, || Bucket::at(1).available(UNEVEN))
}

#[test]
fn refills_add_up_however_often_they_happen() {
    let once = refilled_after(&[]);
    assert_eq!(once, 231);
    assert_eq!(refilled_after(&[2, 3, 4, 5, 50, 51, 99]), once);
    assert_eq!(refilled_after(&(2..100).collect::<Vec<_>>()), once);
}

// Two takes in one transaction, in a block where no whole unit has flowed
// in: the rule says "nothing to record" both times, and both takes count.
#[test]
fn two_takes_in_a_transaction_between_units() {
    const SLOW: Rate = Rate {
        burst: 10,
        refill: 1,
        period: 60,
    };
    let env = start();
    run(&env, || Bucket::at(1).fill(SLOW));
    at(&env, 1);
    let both = run(&env, || {
        let mut bucket = Bucket::at(1);
        bucket.take(SLOW, 3) && Bucket::at(1).take(SLOW, 4)
    });
    assert!(both);
    env.advance_block();
    assert_eq!(run(&env, || Bucket::at(1).available(SLOW)), 3);
}

mod crowded_bucket {
    use super::*;

    pub fn drain() {
        let mut bucket = Bucket::at(1);
        bucket.fill(PER_SECOND);
        assert!(bucket.take(PER_SECOND, 100));
    }

    // Window 2 is two hours on: the bucket is full again.
    pub fn take(i: usize) {
        assert!(Bucket::at(1).take(PER_SECOND, i as i128 % 3));
    }

    pub fn read() -> i128 {
        Bucket::at(1).available(PER_SECOND)
    }
}

#[test]
fn a_crowd_takes_from_a_refilled_bucket() {
    use crowded_bucket::*;
    let taken: i128 = (0..CROWD as i128).map(|i| i % 3).sum();
    // The read is a block, and so a unit, later; the bucket is not full.
    assert_eq!(crowd_agrees(drain, take, read), 100 - taken + 1);
}

// --- Several counters in one integer ---------------------------------------
//
// The value is opaque to `Rollover`, so a contract can still pack lanes
// into it. Here: units of paint in the top lane, and their summed shade
// below. The lane arithmetic stays the contract's.

mod lanes {
    use super::*;

    const DEPTH: u32 = 64;

    // As a contract would declare it.
    #[derive(Storage)]
    struct Canvas {
        coats: Map<u32, Rollover<Packed>>,
    }

    fn canvas() -> Canvas {
        Keypath::default().child(&3u32).into()
    }

    pub fn old_paint() {
        paint(0);
    }

    pub fn paint(i: usize) {
        let (amount, shade) = (i as i128 + 1, 200 - i as i128);
        canvas()
            .coats
            .at_mut(&9)
            .enter(window(), 0)
            .add((amount << DEPTH) | (amount * shade));
    }

    // The mean shade, weighted by amount.
    pub fn shade() -> i128 {
        let coat = canvas().coats.at(&9).peek(window(), 0);
        (coat & ((1 << DEPTH) - 1)) / (coat >> DEPTH).max(1)
    }
}

#[test]
fn a_crowd_adds_to_packed_lanes() {
    let weighted: i128 = (0..CROWD as i128).map(|i| (i + 1) * (200 - i)).sum();
    assert_eq!(
        crowd_agrees(lanes::old_paint, lanes::paint, lanes::shade),
        weighted / CROWD_SUM
    );
}

// `Packed` keeps the period modulo `2^32 - 1`; a zero tag is a cell never
// written. Periods on either side of the wrap are still distinct.
#[test]
fn packed_periods_wrap_without_meeting_zero() {
    let env = start();
    let wrap = u64::from(u32::MAX);
    run(&env, move || {
        let mut cell = cell::<Packed>();
        cell.enter(Period::from_raw(wrap - 1), 0).add(5);
        assert_eq!(cell.recorded(), (Some(wrap - 1), 5));
    });
    env.advance_block();
    run(&env, move || {
        let mut cell = cell::<Packed>();
        assert_eq!(cell.peek(Period::from_raw(wrap - 1), 0), 5);
        assert_eq!(cell.enter(Period::from_raw(wrap), 0).get(), 0);
        assert_eq!(cell.recorded(), (Some(0), 0));
    });
}
