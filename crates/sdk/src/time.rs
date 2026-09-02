//! Durations, and arithmetic on [`TimePoint`], so a deadline reads the way
//! it does in Solidity:
//!
//! ```
//! use sdk::time::Duration;
//! use sdk::{TimePoint, TimePointExt};
//!
//! let now = TimePoint::from(1_700_000_000 * Duration::SECOND.as_nanos());
//! let deadline = &now + 7 * Duration::DAY;
//!
//! assert!(now < deadline);
//! assert_eq!(deadline.duration_since(&now), Duration::from_days(7));
//! assert_eq!(&deadline - Duration::WEEK, now);
//! ```
//!
//! [`Duration`] is the SDK's counterpart to [`core::time::Duration`], with
//! the same constructors, accessors, operators, and overflow behavior. It is
//! a distinct type for two reasons:
//!
//! - **Wire format.** Contracts store durations and take them as arguments,
//!   so a duration needs an XDR encoding. `Duration` is a single `u64` of
//!   nanoseconds -- the same unit and encoding as [`TimePoint`] -- where
//!   `core`'s is a seconds/nanoseconds pair.
//! - **Operators on `TimePoint`.** `TimePoint` is defined in `groundhog-xdr`
//!   and `core::time::Duration` in `core`, so under the orphan rule this
//!   crate can only implement `TimePoint + Duration` for a `Duration` of its
//!   own. The same rule is why the difference of two timepoints is spelled
//!   [`later.duration_since(&earlier)`](TimePointExt::duration_since) and
//!   not `later - earlier`.
//!
//! The two convert into each other with `From` / `TryFrom`. Every
//! `core::time::Duration` under about 584 years fits.
//!
//! # Overflow
//!
//! Like `core::time`, and unlike the integer primitives, this arithmetic
//! checks in every profile: an operator that overflows, or a timepoint
//! subtraction that would go before the Unix epoch, panics whether or not
//! `overflow-checks` is on. A deadline that silently wrapped into the past
//! would be a security bug, so a native test that passes says the same thing
//! about the deployed contract. Use the `checked_` / `saturating_` forms to
//! handle the edge instead.
//!
//! [`TimePointExt::duration_since`] saturates to [`Duration::ZERO`] when
//! the other timepoint is later, as `std::time::Instant` does; use
//! [`TimePointExt::checked_duration_since`] to detect that case.

use core::fmt;
use core::iter::Sum;
use core::ops::{
    Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign,
};

use crate::types::TimePoint;
use crate::xdr::{ConstXDRSize, ReadXDR, WriteXDR};

const NANOS_PER_MICRO: u64 = 1_000;
const NANOS_PER_MILLI: u64 = 1_000_000;
const NANOS_PER_SEC: u64 = 1_000_000_000;
const NANOS_PER_MIN: u64 = 60 * NANOS_PER_SEC;
const NANOS_PER_HOUR: u64 = 60 * NANOS_PER_MIN;
const NANOS_PER_DAY: u64 = 24 * NANOS_PER_HOUR;
const NANOS_PER_WEEK: u64 = 7 * NANOS_PER_DAY;

/// The Unix epoch, 1970-01-01T00:00:00Z: the [`TimePoint`] every other one
/// is measured from.
///
/// `UNIX_EPOCH + Duration::from_secs(secs)` builds a timepoint from a Unix
/// seconds value, and `t.duration_since(&UNIX_EPOCH)` reads one back.
pub const UNIX_EPOCH: TimePoint = TimePoint::new(0);

/// A span of time in whole nanoseconds, the unit of [`TimePoint`].
///
/// Construct one from the unit constants (`7 * Duration::DAY`) or the
/// `from_*` constructors (`Duration::from_days(7)`); both are `const`. On the
/// wire and in storage it is one `u64`, so it can be a `#[contract_type]`
/// field, a method argument, or a [`Value`](crate::storage::Value).
///
/// Arithmetic panics on overflow in every profile -- see the
/// [module docs](self#overflow).
#[derive(
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    ReadXDR,
    WriteXDR,
    ConstXDRSize,
)]
#[repr(transparent)]
pub struct Duration {
    nanos: u64,
}

macro_rules! unit_constructor {
    ($(#[$doc:meta])* $name:ident, $unit:expr, $overflow:literal) => {
        $(#[$doc])*
        ///
        /// Panics if the result does not fit in `u64` nanoseconds.
        pub const fn $name(count: u64) -> Duration {
            match count.checked_mul($unit) {
                Some(nanos) => Duration { nanos },
                None => panic!($overflow),
            }
        }
    };
}

impl Duration {
    /// No time at all.
    pub const ZERO: Duration = Duration::from_nanos(0);
    /// The longest representable duration, about 584 years.
    pub const MAX: Duration = Duration::from_nanos(u64::MAX);

    /// One nanosecond.
    pub const NANOSECOND: Duration = Duration::from_nanos(1);
    /// One microsecond.
    pub const MICROSECOND: Duration = Duration::from_nanos(NANOS_PER_MICRO);
    /// One millisecond.
    pub const MILLISECOND: Duration = Duration::from_nanos(NANOS_PER_MILLI);
    /// One second.
    pub const SECOND: Duration = Duration::from_nanos(NANOS_PER_SEC);
    /// One minute.
    pub const MINUTE: Duration = Duration::from_nanos(NANOS_PER_MIN);
    /// One hour.
    pub const HOUR: Duration = Duration::from_nanos(NANOS_PER_HOUR);
    /// One day of 24 hours.
    pub const DAY: Duration = Duration::from_nanos(NANOS_PER_DAY);
    /// One week of 7 days.
    pub const WEEK: Duration = Duration::from_nanos(NANOS_PER_WEEK);

    /// A duration of `nanos` nanoseconds.
    pub const fn from_nanos(nanos: u64) -> Duration {
        Duration { nanos }
    }

    unit_constructor!(
        /// A duration of `count` microseconds.
        from_micros,
        NANOS_PER_MICRO,
        "overflow in Duration::from_micros"
    );
    unit_constructor!(
        /// A duration of `count` milliseconds.
        from_millis,
        NANOS_PER_MILLI,
        "overflow in Duration::from_millis"
    );
    unit_constructor!(
        /// A duration of `count` seconds.
        from_secs,
        NANOS_PER_SEC,
        "overflow in Duration::from_secs"
    );
    unit_constructor!(
        /// A duration of `count` minutes.
        from_mins,
        NANOS_PER_MIN,
        "overflow in Duration::from_mins"
    );
    unit_constructor!(
        /// A duration of `count` hours.
        from_hours,
        NANOS_PER_HOUR,
        "overflow in Duration::from_hours"
    );
    unit_constructor!(
        /// A duration of `count` days of 24 hours.
        from_days,
        NANOS_PER_DAY,
        "overflow in Duration::from_days"
    );
    unit_constructor!(
        /// A duration of `count` weeks of 7 days.
        from_weeks,
        NANOS_PER_WEEK,
        "overflow in Duration::from_weeks"
    );

    /// The whole duration in nanoseconds.
    ///
    /// A `u64`, not the `u128` of `core::time::Duration::as_nanos`: this type
    /// cannot hold more.
    pub const fn as_nanos(self) -> u64 {
        self.nanos
    }

    /// The duration in whole microseconds, rounding down.
    pub const fn as_micros(self) -> u64 {
        self.nanos / NANOS_PER_MICRO
    }

    /// The duration in whole milliseconds, rounding down.
    pub const fn as_millis(self) -> u64 {
        self.nanos / NANOS_PER_MILLI
    }

    /// The duration in whole seconds, rounding down.
    pub const fn as_secs(self) -> u64 {
        self.nanos / NANOS_PER_SEC
    }

    /// The fractional part of the duration in nanoseconds (always below
    /// one second).
    pub const fn subsec_nanos(self) -> u32 {
        (self.nanos % NANOS_PER_SEC) as u32
    }

    /// The fractional part of the duration in whole microseconds.
    pub const fn subsec_micros(self) -> u32 {
        self.subsec_nanos() / NANOS_PER_MICRO as u32
    }

    /// The fractional part of the duration in whole milliseconds.
    pub const fn subsec_millis(self) -> u32 {
        self.subsec_nanos() / NANOS_PER_MILLI as u32
    }

    /// Whether this is [`Duration::ZERO`].
    pub const fn is_zero(self) -> bool {
        self.nanos == 0
    }

    /// `self + rhs`, or `None` on overflow.
    pub const fn checked_add(self, rhs: Duration) -> Option<Duration> {
        match self.nanos.checked_add(rhs.nanos) {
            Some(nanos) => Some(Duration { nanos }),
            None => None,
        }
    }

    /// `self - rhs`, or `None` if `rhs` is longer.
    pub const fn checked_sub(self, rhs: Duration) -> Option<Duration> {
        match self.nanos.checked_sub(rhs.nanos) {
            Some(nanos) => Some(Duration { nanos }),
            None => None,
        }
    }

    /// `self * rhs`, or `None` on overflow.
    pub const fn checked_mul(self, rhs: u64) -> Option<Duration> {
        match self.nanos.checked_mul(rhs) {
            Some(nanos) => Some(Duration { nanos }),
            None => None,
        }
    }

    /// `self / rhs` rounding down, or `None` if `rhs` is zero.
    pub const fn checked_div(self, rhs: u64) -> Option<Duration> {
        match self.nanos.checked_div(rhs) {
            Some(nanos) => Some(Duration { nanos }),
            None => None,
        }
    }

    /// `self + rhs`, or [`Duration::MAX`] on overflow.
    pub const fn saturating_add(self, rhs: Duration) -> Duration {
        Duration {
            nanos: self.nanos.saturating_add(rhs.nanos),
        }
    }

    /// `self - rhs`, or [`Duration::ZERO`] if `rhs` is longer.
    pub const fn saturating_sub(self, rhs: Duration) -> Duration {
        Duration {
            nanos: self.nanos.saturating_sub(rhs.nanos),
        }
    }

    /// `self * rhs`, or [`Duration::MAX`] on overflow.
    pub const fn saturating_mul(self, rhs: u64) -> Duration {
        Duration {
            nanos: self.nanos.saturating_mul(rhs),
        }
    }
}

impl fmt::Debug for Duration {
    /// Formats as `core::time::Duration` does: `1.5s`, `250ms`, `7ns`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&core::time::Duration::from(*self), f)
    }
}

impl From<Duration> for core::time::Duration {
    fn from(duration: Duration) -> Self {
        core::time::Duration::from_nanos(duration.nanos)
    }
}

/// The error of converting a [`core::time::Duration`] too long for
/// [`Duration`] -- above `u64::MAX` nanoseconds, about 584 years.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("duration exceeds the u64 nanosecond range")]
pub struct TryFromDurationError(());

impl TryFrom<core::time::Duration> for Duration {
    type Error = TryFromDurationError;

    fn try_from(duration: core::time::Duration) -> Result<Self, Self::Error> {
        u64::try_from(duration.as_nanos())
            .map(Duration::from_nanos)
            .map_err(|_| TryFromDurationError(()))
    }
}

impl Add for Duration {
    type Output = Duration;

    fn add(self, rhs: Duration) -> Duration {
        match self.checked_add(rhs) {
            Some(sum) => sum,
            None => panic!("overflow when adding durations"),
        }
    }
}

impl AddAssign for Duration {
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs;
    }
}

impl Sub for Duration {
    type Output = Duration;

    fn sub(self, rhs: Duration) -> Duration {
        match self.checked_sub(rhs) {
            Some(difference) => difference,
            None => panic!("overflow when subtracting durations"),
        }
    }
}

impl SubAssign for Duration {
    fn sub_assign(&mut self, rhs: Duration) {
        *self = *self - rhs;
    }
}

impl Mul<u64> for Duration {
    type Output = Duration;

    fn mul(self, rhs: u64) -> Duration {
        match self.checked_mul(rhs) {
            Some(product) => product,
            None => panic!("overflow when multiplying duration by scalar"),
        }
    }
}

impl Mul<Duration> for u64 {
    type Output = Duration;

    fn mul(self, rhs: Duration) -> Duration {
        rhs * self
    }
}

impl MulAssign<u64> for Duration {
    fn mul_assign(&mut self, rhs: u64) {
        *self = *self * rhs;
    }
}

impl Div<u64> for Duration {
    type Output = Duration;

    fn div(self, rhs: u64) -> Duration {
        match self.checked_div(rhs) {
            Some(quotient) => quotient,
            None => {
                panic!("divide by zero error when dividing duration by scalar")
            }
        }
    }
}

impl DivAssign<u64> for Duration {
    fn div_assign(&mut self, rhs: u64) {
        *self = *self / rhs;
    }
}

impl Sum for Duration {
    fn sum<I: Iterator<Item = Duration>>(iter: I) -> Duration {
        iter.fold(Duration::ZERO, Add::add)
    }
}

impl<'a> Sum<&'a Duration> for Duration {
    fn sum<I: Iterator<Item = &'a Duration>>(iter: I) -> Duration {
        iter.copied().sum()
    }
}

// A duration is one `u64` on the wire, exactly as `TimePoint` is; the spec
// says so directly rather than naming a typedef, as `BlockHeight` does.
impl crate::__contract_spec::ContractTypeRefSpec for Duration {
    const INFO: crate::__contract_spec::ContractTypeInfo<'static> =
        crate::__contract_spec::ContractTypeInfo {
            ty: crate::__contract_spec::ContractTypeView::U64,
        };
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::TimePoint {}
}

/// Checked and saturating arithmetic on [`TimePoint`], alongside the `+` and
/// `-` operators.
///
/// The operator forms panic where these return `None` or saturate; see the
/// [module docs](self#overflow). This trait is sealed: it exists to give the
/// wire type methods, not to be implemented elsewhere.
pub trait TimePointExt: sealed::Sealed {
    /// `self + duration`, or `None` if that is past [`TimePoint`]'s range.
    fn checked_add(&self, duration: Duration) -> Option<TimePoint>;

    /// `self - duration`, or `None` if that is before the Unix epoch.
    fn checked_sub(&self, duration: Duration) -> Option<TimePoint>;

    /// `self + duration`, or the latest representable timepoint on
    /// overflow.
    ///
    /// Useful for a deadline that a caller-supplied span may push past the
    /// end of the range: the result is simply "never".
    fn saturating_add(&self, duration: Duration) -> TimePoint;

    /// `self - duration`, or [`UNIX_EPOCH`] if that would precede it.
    fn saturating_sub(&self, duration: Duration) -> TimePoint;

    /// The time from `earlier` to `self`, or [`Duration::ZERO`] if `earlier`
    /// is in fact later.
    fn duration_since(&self, earlier: &TimePoint) -> Duration;

    /// The time from `earlier` to `self`, or `None` if `earlier` is in fact
    /// later.
    fn checked_duration_since(&self, earlier: &TimePoint) -> Option<Duration>;
}

impl TimePointExt for TimePoint {
    fn checked_add(&self, duration: Duration) -> Option<TimePoint> {
        (**self).checked_add(duration.nanos).map(TimePoint::new)
    }

    fn checked_sub(&self, duration: Duration) -> Option<TimePoint> {
        (**self).checked_sub(duration.nanos).map(TimePoint::new)
    }

    fn saturating_add(&self, duration: Duration) -> TimePoint {
        TimePoint::new((**self).saturating_add(duration.nanos))
    }

    fn saturating_sub(&self, duration: Duration) -> TimePoint {
        TimePoint::new((**self).saturating_sub(duration.nanos))
    }

    fn duration_since(&self, earlier: &TimePoint) -> Duration {
        Duration::from_nanos((**self).saturating_sub(**earlier))
    }

    fn checked_duration_since(&self, earlier: &TimePoint) -> Option<Duration> {
        (**self).checked_sub(**earlier).map(Duration::from_nanos)
    }
}

impl Add<Duration> for &TimePoint {
    type Output = TimePoint;

    fn add(self, rhs: Duration) -> TimePoint {
        match self.checked_add(rhs) {
            Some(later) => later,
            None => panic!("overflow when adding duration to timepoint"),
        }
    }
}

impl Add<Duration> for TimePoint {
    type Output = TimePoint;

    fn add(self, rhs: Duration) -> TimePoint {
        &self + rhs
    }
}

impl AddAssign<Duration> for TimePoint {
    fn add_assign(&mut self, rhs: Duration) {
        *self = &*self + rhs;
    }
}

impl Sub<Duration> for &TimePoint {
    type Output = TimePoint;

    fn sub(self, rhs: Duration) -> TimePoint {
        match self.checked_sub(rhs) {
            Some(earlier) => earlier,
            None => {
                panic!("overflow when subtracting duration from timepoint")
            }
        }
    }
}

impl Sub<Duration> for TimePoint {
    type Output = TimePoint;

    fn sub(self, rhs: Duration) -> TimePoint {
        &self - rhs
    }
}

impl SubAssign<Duration> for TimePoint {
    fn sub_assign(&mut self, rhs: Duration) {
        *self = &*self - rhs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal::encode_xdr;

    fn timepoint(nanos: u64) -> TimePoint {
        TimePoint::new(nanos)
    }

    #[test]
    fn unit_constants_and_constructors_agree() {
        assert_eq!(Duration::from_micros(3), 3 * Duration::MICROSECOND);
        assert_eq!(Duration::from_millis(3), 3 * Duration::MILLISECOND);
        assert_eq!(Duration::from_secs(3), 3 * Duration::SECOND);
        assert_eq!(Duration::from_mins(3), 3 * Duration::MINUTE);
        assert_eq!(Duration::from_hours(3), 3 * Duration::HOUR);
        assert_eq!(Duration::from_days(3), 3 * Duration::DAY);
        assert_eq!(Duration::from_weeks(3), 3 * Duration::WEEK);
        assert_eq!(Duration::WEEK, Duration::from_days(7));
        assert_eq!(Duration::DAY, Duration::from_hours(24));
        assert_eq!(Duration::HOUR, Duration::from_mins(60));
        assert_eq!(Duration::MINUTE, Duration::from_secs(60));
        assert_eq!(Duration::SECOND.as_nanos(), 1_000_000_000);
    }

    #[test]
    fn accessors_round_down_and_split_like_core() {
        let d = Duration::from_nanos(1_500_000_001);
        let core = core::time::Duration::from(d);
        assert_eq!(d.as_secs(), core.as_secs());
        assert_eq!(d.as_millis(), core.as_millis() as u64);
        assert_eq!(d.as_micros(), core.as_micros() as u64);
        assert_eq!(d.as_nanos(), core.as_nanos() as u64);
        assert_eq!(d.subsec_nanos(), core.subsec_nanos());
        assert_eq!(d.subsec_micros(), core.subsec_micros());
        assert_eq!(d.subsec_millis(), core.subsec_millis());
        assert!(Duration::ZERO.is_zero());
        assert!(!d.is_zero());
    }

    #[test]
    fn core_conversions_roundtrip_and_reject_overflow() {
        let d = Duration::from_millis(1_234);
        let core: core::time::Duration = d.into();
        assert_eq!(core, core::time::Duration::from_millis(1_234));
        assert_eq!(Duration::try_from(core), Ok(d));

        let max: core::time::Duration = Duration::MAX.into();
        assert_eq!(Duration::try_from(max), Ok(Duration::MAX));
        assert_eq!(
            Duration::try_from(max + core::time::Duration::from_nanos(1)),
            Err(TryFromDurationError(()))
        );
    }

    #[test]
    fn duration_operators_and_checked_forms() {
        let d = Duration::from_secs(90);
        assert_eq!(d + Duration::from_secs(30), Duration::from_mins(2));
        assert_eq!(d - Duration::from_secs(30), Duration::from_mins(1));
        assert_eq!(d * 2, Duration::from_mins(3));
        assert_eq!(2 * d, Duration::from_mins(3));
        assert_eq!(d / 3, Duration::from_secs(30));

        let mut acc = d;
        acc += Duration::from_secs(30);
        acc -= Duration::from_secs(60);
        acc *= 4;
        acc /= 2;
        assert_eq!(acc, Duration::from_mins(2));

        assert_eq!(Duration::MAX.checked_add(Duration::NANOSECOND), None);
        assert_eq!(Duration::ZERO.checked_sub(Duration::NANOSECOND), None);
        assert_eq!(Duration::MAX.checked_mul(2), None);
        assert_eq!(d.checked_div(0), None);
        assert_eq!(
            Duration::MAX.saturating_add(Duration::SECOND),
            Duration::MAX
        );
        assert_eq!(
            Duration::ZERO.saturating_sub(Duration::SECOND),
            Duration::ZERO
        );
        assert_eq!(Duration::MAX.saturating_mul(3), Duration::MAX);

        let total: Duration = [d, d, d].iter().sum();
        assert_eq!(total, d * 3);
        let total: Duration = [d, d].into_iter().sum();
        assert_eq!(total, d * 2);
    }

    #[test]
    #[should_panic(expected = "overflow when adding durations")]
    fn duration_add_overflow_panics() {
        let _ = Duration::MAX + Duration::NANOSECOND;
    }

    #[test]
    #[should_panic(expected = "overflow when subtracting durations")]
    fn duration_sub_overflow_panics() {
        let _ = Duration::ZERO - Duration::NANOSECOND;
    }

    #[test]
    #[should_panic(expected = "overflow in Duration::from_weeks")]
    fn unit_constructor_overflow_panics() {
        let _ = Duration::from_weeks(u64::MAX);
    }

    #[test]
    fn timepoint_plus_and_minus_duration() {
        let now = timepoint(1_000);
        let deadline = &now + 7 * Duration::NANOSECOND;
        assert_eq!(deadline, timepoint(1_007));
        assert!(now < deadline);
        assert_eq!(deadline.clone() - Duration::from_nanos(7), now);
        assert_eq!(&deadline - Duration::from_nanos(7), now);

        let mut moving = now.clone();
        moving += Duration::from_nanos(10);
        moving -= Duration::from_nanos(3);
        assert_eq!(moving, timepoint(1_007));

        assert_eq!(UNIX_EPOCH + Duration::from_secs(5), timepoint(5e9 as u64));
    }

    #[test]
    fn timepoint_difference_is_a_saturating_duration() {
        let start = timepoint(100);
        let end = timepoint(250);
        assert_eq!(end.duration_since(&start), Duration::from_nanos(150));
        assert_eq!(
            end.checked_duration_since(&start),
            Some(Duration::from_nanos(150))
        );

        assert_eq!(start.duration_since(&end), Duration::ZERO);
        assert_eq!(start.checked_duration_since(&end), None);
    }

    #[test]
    fn timepoint_checked_and_saturating_forms() {
        let late = timepoint(u64::MAX - 1);
        assert_eq!(
            late.checked_add(Duration::from_nanos(1)),
            Some(timepoint(u64::MAX))
        );
        assert_eq!(late.checked_add(Duration::from_nanos(2)), None);
        assert_eq!(late.saturating_add(Duration::WEEK), timepoint(u64::MAX));

        let early = timepoint(1);
        assert_eq!(
            early.checked_sub(Duration::from_nanos(1)),
            Some(UNIX_EPOCH)
        );
        assert_eq!(early.checked_sub(Duration::from_nanos(2)), None);
        assert_eq!(early.saturating_sub(Duration::WEEK), UNIX_EPOCH);
    }

    #[test]
    #[should_panic(expected = "overflow when adding duration to timepoint")]
    fn timepoint_add_overflow_panics() {
        let _ = timepoint(u64::MAX) + Duration::NANOSECOND;
    }

    #[test]
    #[should_panic(
        expected = "overflow when subtracting duration from timepoint"
    )]
    fn timepoint_sub_before_epoch_panics() {
        let _ = UNIX_EPOCH - Duration::NANOSECOND;
    }

    #[test]
    fn encodes_as_one_u64_like_timepoint() {
        let d = Duration::from_secs(1_700_000_000);
        let as_timepoint = timepoint(d.as_nanos());
        let encoded = encode_xdr(&d).unwrap();
        assert_eq!(&encoded[..], &encode_xdr(&as_timepoint).unwrap()[..]);

        let mut reader = xdrs::cxs::buf_reader::BufReader::new(&encoded[..]);
        let decoded = Duration::read_xdr(&mut reader).unwrap();
        assert_eq!(decoded, d);
    }

    #[test]
    fn debug_formats_like_core() {
        assert_eq!(std::format!("{:?}", Duration::from_millis(1_500)), "1.5s");
        assert_eq!(std::format!("{:?}", Duration::from_nanos(7)), "7ns");
    }
}
