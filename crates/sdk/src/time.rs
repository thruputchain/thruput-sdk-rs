//! Time in a contract: the block's [`TimePoint`], spans between timepoints,
//! and periods counted from an origin.
//!
//! [`get_block_timepoint`](crate::block::get_block_timepoint) returns a
//! [`TimePoint`]. Keep it as one. A `TimePoint` is a `#[contract_type]`
//! field, a `#[contract]` method parameter and a stored value, and it
//! compares with `<` and `>=`, so a deadline needs no integer:
//!
//! ```
//! # #![allow(unexpected_cfgs)]
//! use thruput_sdk::*;
//!
//! #[contract_type]
//! #[derive(Clone, Debug, PartialEq, Eq)]
//! pub struct Sale {
//!     /// The first block that takes no more bids.
//!     pub end: TimePoint,
//! }
//!
//! fn is_open(sale: &Sale) -> bool {
//!     get_block_timepoint() < sale.end
//! }
//! # fn main() {}
//! ```
//!
//! A length of time is a [`TimeSpan`]. It is the type to store, to take as a
//! parameter and to put in a `#[contract_type]`, where
//! [`Duration`] cannot go: `core`'s type has no XDR form.
//! A `TimeSpan` adds to and subtracts from a `TimePoint`, and
//! [`TimeSpan::between`] measures from one timepoint to another:
//!
//! ```
//! # #![allow(unexpected_cfgs)]
//! use thruput_sdk::*;
//!
//! #[contract_type]
//! #[derive(Clone, Copy, Debug, PartialEq, Eq)]
//! pub struct Feed {
//!     /// How old a price may be.
//!     pub max_age: TimeSpan,
//! }
//!
//! fn is_fresh(feed: &Feed, observed: &TimePoint, now: &TimePoint) -> bool {
//!     observed <= now && TimeSpan::between(observed, now) <= feed.max_age
//! }
//! # fn main() {}
//! ```
//!
//! For a clock that runs in equal periods from an origin (an epoch, a
//! vesting step, a rate-limit window), [`period_at`] gives the period a
//! timepoint falls in and how far into it:
//!
//! ```
//! use thruput_sdk::time::{Period, period_at};
//! use thruput_sdk::{TimePoint, TimeSpan};
//!
//! const EPOCH: TimeSpan = TimeSpan::from_mins(1);
//! let origin = TimePoint::UNIX_EPOCH + TimeSpan::from_days(30_000);
//! let now = &origin + TimeSpan::from_secs(150);
//!
//! assert_eq!(
//!     period_at(&origin, EPOCH, &now),
//!     Some(Period { index: 2, elapsed: TimeSpan::from_secs(30) })
//! );
//! ```
//!
//! # Overflow
//!
//! The operators on `TimePoint` and `TimeSpan` panic on overflow in every
//! build profile, unlike the integer primitives. A deadline that wrapped
//! into the past would be a security bug. The `checked_` and `saturating_`
//! methods handle the edge without a panic. Use them on spans a caller
//! supplies.
//!
//! # Constants
//!
//! The `TimePoint` operators are not `const`. Every `TimeSpan` constructor
//! is a `const fn`, and so is [`TimeSpan::after_unix_epoch`], which builds a
//! constant timepoint:
//!
//! ```
//! use thruput_sdk::{TimePoint, TimeSpan};
//!
//! const START: TimePoint = TimeSpan::from_days(30_000).after_unix_epoch();
//! const HOUR: TimeSpan = TimeSpan::from_hours(1);
//! # assert!(START < START.clone() + HOUR);
//! ```
//!
//! # `TimePoint` is not `Copy`
//!
//! A struct that holds a `TimePoint` cannot derive `Copy`. Derive `Clone`,
//! and pass timepoints by reference: the operators and methods here take
//! `&TimePoint`. A `TimeSpan` is `Copy`.

use core::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};
use core::time::Duration;

use crate::call::OwnedRefArg;
use crate::collect_auth_handles::{AuthHandleBuf, CollectAuthHandles};
use crate::mintable::{ChildNodeBuf, Mintable, MintableType};
use crate::receive_auth_handles::ReceiveAuthHandles;
use crate::types::TimePoint;
use crate::xdr::{
    ReadResult, ReadXDR, Reader, WriteResult, WriteXDR, Writer,
    WriterUnchecked, cxs, txs,
};
use crate::{AuthTokenError, FaultString};

const NANOS_PER_SEC: u64 = 1_000_000_000;

/// A length of time in whole nanoseconds, the unit of [`TimePoint`].
///
/// Use it wherever a length of time is stored, passed to a `#[contract]`
/// method or held in a `#[contract_type]`. Its wire and storage form is one
/// `u64` of nanoseconds, and the contract spec shows it as `u64`. The
/// longest span is [`TimeSpan::MAX`], about 584 years.
///
/// It converts to [`Duration`] with `into()`, and from one
/// with `try_into()`, which fails past `TimeSpan::MAX`. See the
/// [module documentation](self) for examples.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[repr(transparent)]
pub struct TimeSpan {
    nanos: u64,
}

/// A [`Duration`] too long for a [`TimeSpan`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeSpanOverflow;

impl core::fmt::Display for TimeSpanOverflow {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("duration does not fit in u64 nanoseconds")
    }
}

impl core::error::Error for TimeSpanOverflow {}

const fn scaled(count: u64, nanos_each: u64) -> TimeSpan {
    match count.checked_mul(nanos_each) {
        Some(nanos) => TimeSpan { nanos },
        None => panic!("overflow in TimeSpan constructor"),
    }
}

impl TimeSpan {
    /// No time at all.
    pub const ZERO: TimeSpan = TimeSpan { nanos: 0 };

    /// The longest span: `u64::MAX` nanoseconds, about 584 years.
    pub const MAX: TimeSpan = TimeSpan { nanos: u64::MAX };

    /// A span of `nanos` nanoseconds.
    pub const fn from_nanos(nanos: u64) -> TimeSpan {
        TimeSpan { nanos }
    }

    /// A span of `millis` milliseconds. Panics past [`TimeSpan::MAX`].
    pub const fn from_millis(millis: u64) -> TimeSpan {
        scaled(millis, NANOS_PER_SEC / 1_000)
    }

    /// A span of `secs` seconds. Panics past [`TimeSpan::MAX`].
    pub const fn from_secs(secs: u64) -> TimeSpan {
        scaled(secs, NANOS_PER_SEC)
    }

    /// A span of `mins` minutes. Panics past [`TimeSpan::MAX`].
    pub const fn from_mins(mins: u64) -> TimeSpan {
        scaled(mins, 60 * NANOS_PER_SEC)
    }

    /// A span of `hours` hours. Panics past [`TimeSpan::MAX`].
    pub const fn from_hours(hours: u64) -> TimeSpan {
        scaled(hours, 3_600 * NANOS_PER_SEC)
    }

    /// A span of `days` 24-hour days. Panics past [`TimeSpan::MAX`].
    pub const fn from_days(days: u64) -> TimeSpan {
        scaled(days, 86_400 * NANOS_PER_SEC)
    }

    /// The span as long as `duration`, or `None` past [`TimeSpan::MAX`].
    ///
    /// The `const` form of `TimeSpan::try_from(duration)`.
    pub const fn from_duration(duration: Duration) -> Option<TimeSpan> {
        let nanos = duration.as_nanos();
        if nanos > u64::MAX as u128 {
            None
        } else {
            Some(TimeSpan {
                nanos: nanos as u64,
            })
        }
    }

    /// The same length of time as a [`Duration`].
    pub const fn as_duration(self) -> Duration {
        Duration::from_nanos(self.nanos)
    }

    /// The span in nanoseconds.
    pub const fn as_nanos(self) -> u64 {
        self.nanos
    }

    /// The whole seconds in the span.
    pub const fn as_secs(self) -> u64 {
        self.nanos / NANOS_PER_SEC
    }

    /// Whether the span is [`TimeSpan::ZERO`].
    pub const fn is_zero(self) -> bool {
        self.nanos == 0
    }

    /// The time from `earlier` to `later`, or [`TimeSpan::ZERO`] if `earlier`
    /// is in fact later.
    ///
    /// The same span as `later - earlier`, which is a
    /// [`Duration`].
    pub fn between(earlier: &TimePoint, later: &TimePoint) -> TimeSpan {
        TimeSpan {
            nanos: (**later).saturating_sub(**earlier),
        }
    }

    /// The time from `earlier` to `later`, or `None` if `earlier` is in fact
    /// later.
    pub fn checked_between(
        earlier: &TimePoint,
        later: &TimePoint,
    ) -> Option<TimeSpan> {
        (**later).checked_sub(**earlier).map(TimeSpan::from_nanos)
    }

    /// The timepoint this long after the Unix epoch.
    ///
    /// A `const fn`, for a constant timepoint. Elsewhere,
    /// `TimePoint::UNIX_EPOCH + self` reads better.
    pub const fn after_unix_epoch(self) -> TimePoint {
        TimePoint::new(self.nanos)
    }

    /// The time from the Unix epoch to `timepoint`.
    pub fn since_unix_epoch(timepoint: &TimePoint) -> TimeSpan {
        TimeSpan { nanos: **timepoint }
    }

    /// The timepoint this long after `start`. Panics past the end of the
    /// representable range.
    ///
    /// The same as `start + self`.
    pub fn after(self, start: &TimePoint) -> TimePoint {
        self.checked_after(start)
            .expect("overflow when adding time span to timepoint")
    }

    /// The timepoint this long after `start`, or `None` if that is past the
    /// end of the representable range.
    pub fn checked_after(self, start: &TimePoint) -> Option<TimePoint> {
        (**start).checked_add(self.nanos).map(TimePoint::new)
    }

    /// The timepoint this long after `start`, or the latest representable
    /// timepoint if that overflows.
    ///
    /// The form for a deadline that a caller-supplied span may push past the
    /// end of the range: the result then reads as "never".
    pub fn saturating_after(self, start: &TimePoint) -> TimePoint {
        TimePoint::new((**start).saturating_add(self.nanos))
    }

    /// The timepoint this long before `end`. Panics if that precedes the Unix
    /// epoch.
    ///
    /// The same as `end - self`.
    pub fn before(self, end: &TimePoint) -> TimePoint {
        self.checked_before(end)
            .expect("overflow when subtracting time span from timepoint")
    }

    /// The timepoint this long before `end`, or `None` if that precedes the
    /// Unix epoch.
    pub fn checked_before(self, end: &TimePoint) -> Option<TimePoint> {
        (**end).checked_sub(self.nanos).map(TimePoint::new)
    }

    /// `self + other`, or `None` past [`TimeSpan::MAX`].
    pub const fn checked_add(self, other: TimeSpan) -> Option<TimeSpan> {
        match self.nanos.checked_add(other.nanos) {
            Some(nanos) => Some(TimeSpan { nanos }),
            None => None,
        }
    }

    /// `self - other`, or `None` if `other` is the longer.
    pub const fn checked_sub(self, other: TimeSpan) -> Option<TimeSpan> {
        match self.nanos.checked_sub(other.nanos) {
            Some(nanos) => Some(TimeSpan { nanos }),
            None => None,
        }
    }

    /// `self * count`, or `None` past [`TimeSpan::MAX`].
    pub const fn checked_mul(self, count: u64) -> Option<TimeSpan> {
        match self.nanos.checked_mul(count) {
            Some(nanos) => Some(TimeSpan { nanos }),
            None => None,
        }
    }

    /// How many whole `period`s fit in the span, and what is left over.
    /// `None` if `period` is zero.
    ///
    /// The quotient and remainder of one span by another. See [`period_at`]
    /// to count periods from an origin.
    pub const fn checked_div_rem(
        self,
        period: TimeSpan,
    ) -> Option<(u64, TimeSpan)> {
        if period.nanos == 0 {
            return None;
        }
        Some((
            self.nanos / period.nanos,
            TimeSpan {
                nanos: self.nanos % period.nanos,
            },
        ))
    }
}

/// Where a timepoint falls on a clock of equal periods. See [`period_at`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Period {
    /// How many whole periods have passed: 0 through the first period.
    pub index: u64,

    /// How far into period `index` the timepoint is: less than the period.
    pub elapsed: TimeSpan,
}

/// The period that `at` falls in, on a clock of equal `period`s that starts
/// at `origin`.
///
/// Period `n` runs from `origin + n * period` up to, and not including,
/// `origin + (n + 1) * period`. Returns `None` if `at` is before `origin`,
/// or `period` is zero.
///
/// Every transaction in a block sees the same block timepoint, so they all
/// compute the same [`Period`].
pub fn period_at(
    origin: &TimePoint,
    period: TimeSpan,
    at: &TimePoint,
) -> Option<Period> {
    let (index, elapsed) =
        TimeSpan::checked_between(origin, at)?.checked_div_rem(period)?;
    Some(Period { index, elapsed })
}

impl From<TimeSpan> for Duration {
    fn from(span: TimeSpan) -> Duration {
        span.as_duration()
    }
}

impl TryFrom<Duration> for TimeSpan {
    type Error = TimeSpanOverflow;

    fn try_from(duration: Duration) -> Result<TimeSpan, TimeSpanOverflow> {
        TimeSpan::from_duration(duration).ok_or(TimeSpanOverflow)
    }
}

// Arithmetic. Like the `TimePoint` operators in `thruput-xdr`, these panic on
// overflow in every build profile.

impl Add<TimeSpan> for &TimePoint {
    type Output = TimePoint;

    fn add(self, rhs: TimeSpan) -> TimePoint {
        rhs.after(self)
    }
}

impl Add<TimeSpan> for TimePoint {
    type Output = TimePoint;

    fn add(self, rhs: TimeSpan) -> TimePoint {
        rhs.after(&self)
    }
}

impl AddAssign<TimeSpan> for TimePoint {
    fn add_assign(&mut self, rhs: TimeSpan) {
        *self = rhs.after(self);
    }
}

impl Sub<TimeSpan> for &TimePoint {
    type Output = TimePoint;

    fn sub(self, rhs: TimeSpan) -> TimePoint {
        rhs.before(self)
    }
}

impl Sub<TimeSpan> for TimePoint {
    type Output = TimePoint;

    fn sub(self, rhs: TimeSpan) -> TimePoint {
        rhs.before(&self)
    }
}

impl SubAssign<TimeSpan> for TimePoint {
    fn sub_assign(&mut self, rhs: TimeSpan) {
        *self = rhs.before(self);
    }
}

impl Add for TimeSpan {
    type Output = TimeSpan;

    fn add(self, rhs: TimeSpan) -> TimeSpan {
        self.checked_add(rhs)
            .expect("overflow when adding time spans")
    }
}

impl AddAssign for TimeSpan {
    fn add_assign(&mut self, rhs: TimeSpan) {
        *self = *self + rhs;
    }
}

impl Sub for TimeSpan {
    type Output = TimeSpan;

    fn sub(self, rhs: TimeSpan) -> TimeSpan {
        self.checked_sub(rhs)
            .expect("overflow when subtracting time spans")
    }
}

impl SubAssign for TimeSpan {
    fn sub_assign(&mut self, rhs: TimeSpan) {
        *self = *self - rhs;
    }
}

impl Mul<u64> for TimeSpan {
    type Output = TimeSpan;

    fn mul(self, rhs: u64) -> TimeSpan {
        self.checked_mul(rhs)
            .expect("overflow when multiplying time span")
    }
}

impl Mul<TimeSpan> for u64 {
    type Output = TimeSpan;

    fn mul(self, rhs: TimeSpan) -> TimeSpan {
        rhs * self
    }
}

impl Div<u64> for TimeSpan {
    type Output = TimeSpan;

    /// Panics if `rhs` is zero.
    fn div(self, rhs: u64) -> TimeSpan {
        TimeSpan {
            nanos: self.nanos / rhs,
        }
    }
}

// The wire and storage form is a `u64` of nanoseconds, as for `TimePoint`.

impl<'r> ReadXDR<'r> for TimeSpan {
    #[inline]
    fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
        u64::read_xdr(r).map(TimeSpan::from_nanos)
    }
}

impl WriteXDR for TimeSpan {
    #[inline]
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        self.nanos.write_xdr(w)
    }

    #[inline]
    unsafe fn write_xdr_unchecked<F, W: WriterUnchecked<F>>(&self, w: &mut W) {
        // SAFETY: the caller's guarantees for `self` are those for the
        // integer, which is all that is written.
        unsafe { self.nanos.write_xdr_unchecked(w) }
    }
}

impl cxs::ConstMaxSize for TimeSpan {
    const MAX_SIZE: Option<usize> = <u64 as cxs::ConstMaxSize>::MAX_SIZE;
}

impl txs::ConstMaxSize for TimeSpan {
    const MAX_SIZE: Option<usize> = <u64 as txs::ConstMaxSize>::MAX_SIZE;
}

/// The spec type is `u64`: the spec format has no node for a time span.
impl thruput_contract_spec::ContractTypeRefSpec for TimeSpan {
    const INFO: thruput_contract_spec::ContractTypeInfo<'static> =
        <u64 as thruput_contract_spec::ContractTypeRefSpec>::INFO;
}

// Contract parameters and auth tokens treat it as a `u64`.

impl OwnedRefArg for TimeSpan {}

impl CollectAuthHandles for TimeSpan {
    #[inline(always)]
    fn collect_auth_handles(
        &self,
        _out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString> {
        Ok(())
    }
}

impl ReceiveAuthHandles for TimeSpan {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        _handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError> {
        Ok(())
    }
}

impl MintableType for TimeSpan {
    type Preimage = TimeSpan;
}

impl Mintable for TimeSpan {
    fn mint_into_parent(
        preimage: TimeSpan,
        _children: &mut ChildNodeBuf,
    ) -> Self {
        preimage
    }
}
