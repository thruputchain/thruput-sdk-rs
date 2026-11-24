//! `TimePoint` and `TimeSpan` are stored, passed and returned as themselves,
//! encode as a `u64` of nanoseconds, and count periods from an origin.

#![allow(unexpected_cfgs)]

use thruput_contract_spec::ContractTypeRefSpec;
use thruput_sdk::storage::{Map, Value};
use thruput_sdk::time::{Period, TimeSpanOverflow, period_at};
use thruput_sdk::xdr::cxs;
use thruput_sdk::*;

/// Holds a timepoint and a span, with the derives each allows.
#[contract_type]
#[derive(Clone, Debug, PartialEq, Eq)]
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

/// A span alone leaves the struct `Copy`.
#[contract_type]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    pub period: TimeSpan,
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
            origin: schedule.origin.clone(),
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
            &self.origin.get(),
            self.epoch.get(),
            &get_block_timepoint(),
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

const START: TimePoint = TimeSpan::from_days(30_000).after_unix_epoch();
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
    assert_eq!(
        <TimeSpan as cxs::ConstMaxSize>::MAX_SIZE,
        <u64 as cxs::ConstMaxSize>::MAX_SIZE,
    );

    let schedule = Schedule {
        origin: START,
        epoch: EPOCH,
        end: Some(START + 10 * EPOCH),
    };
    let raw = RawSchedule {
        origin: *START,
        epoch: EPOCH.as_nanos(),
        end: Some(*START + 10 * EPOCH.as_nanos()),
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

    let stored = schedule.clone();
    call(&env, move |clock| clock.set_schedule(stored));
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
    let origin = call(&env, |clock| clock.start(EPOCH));
    assert_eq!(origin, START);
    // Commit the origin, in a block at the same timepoint.
    env.advance_block_at(START);
    assert_eq!(call(&env, |clock| clock.epoch_now()), (0, TimeSpan::ZERO));

    env.advance_block_at(START + EPOCH - TimeSpan::from_nanos(1));
    assert_eq!(
        call(&env, |clock| clock.epoch_now()),
        (0, EPOCH - TimeSpan::from_nanos(1))
    );

    env.advance_block_at(START + EPOCH);
    assert_eq!(call(&env, |clock| clock.epoch_now()), (1, TimeSpan::ZERO));

    env.advance_block_at(START + 7 * EPOCH + TimeSpan::from_secs(15));
    assert_eq!(
        call(&env, |clock| clock.epoch_now()),
        (7, TimeSpan::from_secs(15))
    );
}

#[test]
fn period_at_splits_time_since_an_origin() {
    let at = |nanos| period_at(&START, EPOCH, &(START + nanos));
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
    let before = START - TimeSpan::from_nanos(1);
    assert_eq!(period_at(&START, EPOCH, &before), None);
    assert_eq!(period_at(&START, TimeSpan::ZERO, &START), None);

    // The whole range, in the shortest period.
    let last = TimeSpan::MAX.after_unix_epoch();
    assert_eq!(
        period_at(&TimePoint::UNIX_EPOCH, TimeSpan::from_nanos(1), &last),
        Some(Period {
            index: u64::MAX,
            elapsed: TimeSpan::ZERO
        })
    );
}

#[test]
fn spans_build_from_units() {
    assert_eq!(TimeSpan::from_nanos(7).as_nanos(), 7);
    assert_eq!(TimeSpan::from_millis(1_500).as_nanos(), 1_500_000_000);
    assert_eq!(TimeSpan::from_secs(60), TimeSpan::from_mins(1));
    assert_eq!(TimeSpan::from_mins(60), TimeSpan::from_hours(1));
    assert_eq!(TimeSpan::from_hours(24), TimeSpan::from_days(1));
    assert_eq!(TimeSpan::from_millis(2_999).as_secs(), 2);
    assert_eq!(TimeSpan::default(), TimeSpan::ZERO);
    assert!(TimeSpan::ZERO.is_zero());
    assert!(!EPOCH.is_zero());
    assert!(TimeSpan::from_secs(59) < EPOCH && EPOCH < TimeSpan::MAX);
}

#[test]
#[should_panic(expected = "overflow in TimeSpan constructor")]
fn a_span_past_the_range_panics() {
    let _ = TimeSpan::from_days(u64::MAX);
}

#[test]
fn spans_convert_to_and_from_durations() {
    assert_eq!(Duration::from(EPOCH), Duration::from_secs(60));
    assert_eq!(EPOCH.as_duration(), Duration::from_secs(60));
    assert_eq!(TimeSpan::try_from(Duration::from_secs(60)), Ok(EPOCH));
    assert_eq!(
        TimeSpan::try_from(Duration::from_nanos(u64::MAX)),
        Ok(TimeSpan::MAX)
    );

    let too_long = Duration::from_nanos(u64::MAX) + Duration::from_nanos(1);
    assert_eq!(TimeSpan::try_from(too_long), Err(TimeSpanOverflow));
    assert_eq!(TimeSpan::from_duration(Duration::MAX), None);

    // The two kinds of span agree on a timepoint.
    assert_eq!(&START + EPOCH, &START + Duration::from(EPOCH));
    assert_eq!(
        Duration::from(TimeSpan::between(&START, &(START + EPOCH))),
        (START + EPOCH) - START
    );
}

#[test]
fn timepoints_move_by_spans() {
    let later = &START + EPOCH;
    assert_eq!(*later, *START + EPOCH.as_nanos());
    assert_eq!(START + EPOCH, later);
    assert_eq!(EPOCH.after(&START), later);
    assert_eq!(&later - EPOCH, START);
    assert_eq!(later.clone() - EPOCH, START);

    let mut moving = START;
    moving += EPOCH;
    assert_eq!(moving, later);
    moving -= EPOCH;
    assert_eq!(moving, START);

    assert_eq!(TimeSpan::between(&START, &later), EPOCH);
    assert_eq!(TimeSpan::between(&later, &START), TimeSpan::ZERO);
    assert_eq!(TimeSpan::checked_between(&START, &later), Some(EPOCH));
    assert_eq!(TimeSpan::checked_between(&later, &START), None);

    assert_eq!(
        TimeSpan::since_unix_epoch(&START),
        TimeSpan::from_days(30_000)
    );
    assert_eq!(TimeSpan::ZERO.after_unix_epoch(), TimePoint::UNIX_EPOCH);
}

#[test]
fn checked_and_saturating_forms_stop_at_the_range() {
    let last = TimeSpan::MAX.after_unix_epoch();
    let nano = TimeSpan::from_nanos(1);

    assert_eq!(nano.checked_after(&(&last - nano)), Some(last.clone()));
    assert_eq!(nano.checked_after(&last), None);
    assert_eq!(EPOCH.saturating_after(&last), last);
    assert_eq!(EPOCH.saturating_after(&START), START + EPOCH);
    assert_eq!(nano.checked_before(&TimePoint::UNIX_EPOCH), None);
    assert_eq!(
        nano.checked_before(&nano.after_unix_epoch()),
        Some(TimePoint::UNIX_EPOCH)
    );

    assert_eq!(TimeSpan::MAX.checked_add(nano), None);
    assert_eq!(EPOCH.checked_add(EPOCH), Some(TimeSpan::from_mins(2)));
    assert_eq!(TimeSpan::ZERO.checked_sub(nano), None);
    assert_eq!(EPOCH.checked_sub(EPOCH), Some(TimeSpan::ZERO));
    assert_eq!(TimeSpan::MAX.checked_mul(2), None);
    assert_eq!(EPOCH.checked_mul(60), Some(TimeSpan::from_hours(1)));
}

#[test]
fn spans_add_scale_and_divide() {
    let mut span = EPOCH + TimeSpan::from_secs(30);
    assert_eq!(span, TimeSpan::from_secs(90));
    span -= TimeSpan::from_secs(30);
    span += EPOCH;
    assert_eq!(span, 2 * EPOCH);
    assert_eq!(EPOCH * 2, 2 * EPOCH);
    assert_eq!(EPOCH / 4, TimeSpan::from_secs(15));
    assert_eq!(EPOCH - TimeSpan::from_secs(1), TimeSpan::from_secs(59));

    assert_eq!(
        TimeSpan::from_secs(150).checked_div_rem(EPOCH),
        Some((2, TimeSpan::from_secs(30)))
    );
    assert_eq!(EPOCH.checked_div_rem(TimeSpan::ZERO), None);
}

#[test]
#[should_panic(expected = "overflow when adding time span to timepoint")]
fn a_timepoint_past_the_range_panics() {
    let _ = TimeSpan::MAX.after_unix_epoch() + TimeSpan::from_nanos(1);
}

#[test]
#[should_panic(
    expected = "overflow when subtracting time span from timepoint"
)]
fn a_timepoint_before_the_epoch_panics() {
    let _ = TimePoint::UNIX_EPOCH - TimeSpan::from_nanos(1);
}

#[test]
#[should_panic(expected = "overflow when adding time spans")]
fn a_sum_of_spans_past_the_range_panics() {
    let _ = TimeSpan::MAX + TimeSpan::from_nanos(1);
}

#[test]
#[should_panic(expected = "overflow when subtracting time spans")]
fn a_negative_span_panics() {
    let _ = TimeSpan::ZERO - TimeSpan::from_nanos(1);
}

#[test]
#[should_panic(expected = "overflow when multiplying time span")]
fn a_scaled_span_past_the_range_panics() {
    let _ = TimeSpan::MAX * 2;
}
