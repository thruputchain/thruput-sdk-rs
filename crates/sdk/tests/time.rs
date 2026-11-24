//! `TimePoint` and `TimeSpan` are stored, passed and returned as themselves,
//! encode as a `u64`, and count periods from an origin.

#![allow(unexpected_cfgs)]

use thruput_contract_spec::ContractTypeRefSpec;
use thruput_sdk::storage::{Map, Value};
use thruput_sdk::time::{Period, period_at};
use thruput_sdk::xdr::cxs;
use thruput_sdk::*;

/// Holds a timepoint and a span: both are `Copy`.
#[contract_type]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schedule {
    pub origin: TimePoint,
    pub epoch: TimeSpan,
    pub end: Option<TimePoint>,
}

/// The same shape in bare integers.
#[contract_type]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawSchedule {
    pub origin: u64,
    pub epoch: u64,
    pub end: Option<u64>,
}

#[derive(Event, Clone, Debug)]
pub struct Scheduled {
    pub origin: TimePoint,
    pub epoch: TimeSpan,
}

#[thruput_sdk::storage]
pub struct Clock {
    schedule: Value<Schedule>,
    origin: Value<TimePoint>,
    epoch: Value<TimeSpan>,
    seen: Map<TimePoint, Value<TimeSpan>>,
}

#[contract]
impl Clock {
    pub fn set_schedule(&mut self, schedule: Schedule) {
        self.seen.at_mut(&schedule.origin).set(&schedule.epoch);
        log_event(&Scheduled {
            origin: schedule.origin,
            epoch: schedule.epoch,
        });
        self.schedule.set(&schedule);
    }

    pub fn start(&mut self, epoch: TimeSpan) -> TimePoint {
        let now = get_block_timepoint();
        self.origin.set(&now);
        self.epoch.set(&epoch);
        now
    }

    pub fn start_at(&mut self, origin: &TimePoint, epoch: &TimeSpan) {
        self.origin.set(origin);
        self.epoch.set(epoch);
    }

    pub fn schedule(&self) -> Schedule {
        self.schedule.get()
    }

    pub fn origin(&self) -> TimePoint {
        self.origin.get()
    }

    pub fn epoch(&self) -> TimeSpan {
        self.epoch.get()
    }

    pub fn seen(&self, origin: &TimePoint) -> TimeSpan {
        self.seen.at(origin).get()
    }

    /// The epoch this block falls in, and the nanoseconds into it.
    pub fn epoch_now(&self) -> (u64, TimeSpan) {
        let Period { index, elapsed } = period_at(
            self.origin.get(),
            self.epoch.get(),
            get_block_timepoint(),
        )
        .unwrap();
        (index, elapsed)
    }

    pub fn count(&self, spans: &[TimeSpan], points: &[TimePoint]) -> u32 {
        (spans.len() + points.len()) as u32
    }
}

const CLOCK: ContractAddress = ContractAddress::new([0x51; 32]);
const CALLER: ContractAddress = ContractAddress::new([0x52; 32]);

// 2052: after the wall clock, which the environment's first block takes.
const START: TimePoint = TimePoint::new(30_000 * 86_400 * 1_000_000_000);
const EPOCH: TimeSpan = TimeSpan::from_mins(1);

fn environment() -> TestEnvironmentHandle {
    let env = TestEnvironment::no_cost();
    env.register_mock(Clock::__mock).deploy_contract_to(&CLOCK);
    env.advance_block_at(START);
    env
}

fn call<R>(
    env: &TestEnvironmentHandle,
    body: impl FnOnce(ClockClient) -> R + std::panic::UnwindSafe + 'static,
) -> R
where
    R: for<'a> ReadXDR<'a> + WriteXDR + 'static,
{
    let result = env.transact(|tx| {
        tx.call(&CALLER).run(|| body(ClockClient::new(&CLOCK)))
    });
    result.assert_success();
    result.assert_return_as::<R>()
}

#[test]
fn encode_as_u64_nanoseconds() {
    let span = TimeSpan::from_secs(90);
    assert_eq!(
        cxs::encode(&span).unwrap(),
        cxs::encode(&90_000_000_000u64).unwrap()
    );
    assert_eq!(
        cxs::decode::<TimeSpan>(&cxs::encode(&span).unwrap()).unwrap(),
        span
    );
    assert_eq!(START, TimePoint::UNIX_EPOCH + TimeSpan::from_days(30_000));

    let schedule = Schedule {
        origin: START,
        epoch: EPOCH,
        end: Some(START + 10 * EPOCH),
    };
    let raw = RawSchedule {
        origin: *START,
        epoch: *EPOCH,
        end: Some(*START + 10 * *EPOCH),
    };
    let bytes = cxs::encode(&schedule).unwrap();
    assert_eq!(bytes, cxs::encode(&raw).unwrap());
    assert_eq!(cxs::decode::<Schedule>(&bytes).unwrap(), schedule);
}

#[test]
fn have_the_spec_type_u64() {
    let u64_type = <u64 as ContractTypeRefSpec>::INFO.ty;
    assert_eq!(<TimePoint as ContractTypeRefSpec>::INFO.ty, u64_type);
    assert_eq!(<TimeSpan as ContractTypeRefSpec>::INFO.ty, u64_type);
}

#[test]
fn are_stored_passed_and_returned() {
    let env = environment();
    let schedule = Schedule {
        origin: START,
        epoch: EPOCH,
        end: None,
    };

    call(&env, move |clock| clock.set_schedule(schedule));
    env.advance_block();
    assert_eq!(call(&env, |clock| clock.schedule()), schedule);
    assert_eq!(call(&env, |clock| clock.seen(&START)), EPOCH);

    let hour = TimeSpan::from_hours(1);
    call(&env, move |clock| clock.start_at(&(START + hour), &hour));
    env.advance_block();
    assert_eq!(call(&env, |clock| clock.origin()), START + hour);
    assert_eq!(call(&env, |clock| clock.epoch()), hour);

    assert_eq!(
        call(&env, move |clock| clock.count(&[EPOCH, hour], &[START])),
        3
    );
}

#[test]
fn a_contract_counts_epochs_from_its_origin() {
    let env = environment();
    let tick = TimeSpan::new(1);
    let origin = call(&env, |clock| clock.start(EPOCH));
    assert_eq!(origin, START);
    // Commit the origin, in a block at the same timepoint.
    env.advance_block_at(START);
    assert_eq!(call(&env, |clock| clock.epoch_now()), (0, TimeSpan::ZERO));

    env.advance_block_at(START + EPOCH - tick);
    assert_eq!(call(&env, |clock| clock.epoch_now()), (0, EPOCH - tick));

    env.advance_block_at(START + EPOCH);
    assert_eq!(call(&env, |clock| clock.epoch_now()), (1, TimeSpan::ZERO));

    env.advance_block_at(START + 7 * EPOCH + TimeSpan::from_secs(15));
    assert_eq!(
        call(&env, |clock| clock.epoch_now()),
        (7, TimeSpan::from_secs(15))
    );
}

#[test]
fn a_fixed_step_spaces_blocks_by_a_span() {
    let env = environment();
    env.set_block_time_step(Some(EPOCH));
    call(&env, |clock| clock.start(EPOCH));
    for index in 1..=3 {
        env.advance_block();
        assert_eq!(
            call(&env, |clock| clock.epoch_now()),
            (index, TimeSpan::ZERO)
        );
    }
}

#[test]
fn period_at_splits_time_since_an_origin() {
    let at = |since| period_at(START, EPOCH, START + since);
    assert_eq!(
        at(TimeSpan::ZERO),
        Some(Period {
            index: 0,
            elapsed: TimeSpan::ZERO
        })
    );
    assert_eq!(
        at(TimeSpan::from_secs(150)),
        Some(Period {
            index: 2,
            elapsed: TimeSpan::from_secs(30)
        })
    );

    // Before the origin, and with no period to count in.
    assert_eq!(period_at(START, EPOCH, START - TimeSpan::new(1)), None);
    assert_eq!(period_at(START, TimeSpan::ZERO, START), None);

    // The whole range, in the shortest period.
    assert_eq!(
        period_at(
            TimePoint::UNIX_EPOCH,
            TimeSpan::new(1),
            TimePoint::new(u64::MAX)
        ),
        Some(Period {
            index: u64::MAX,
            elapsed: TimeSpan::ZERO
        })
    );
}
