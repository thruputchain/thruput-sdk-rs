//! Time in a contract: the block's [`TimePoint`], the [`TimeSpan`] between
//! two timepoints, and periods counted from an origin.
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
//! #[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
//! A length of time is a [`TimeSpan`]. It is stored and passed as a
//! timepoint is. A span adds to and subtracts from a timepoint, and one
//! timepoint minus another is a span:
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
//! fn is_fresh(feed: &Feed, observed: TimePoint, now: TimePoint) -> bool {
//!     observed <= now && now - observed <= feed.max_age
//! }
//! # fn main() {}
//! ```
//!
//! Build a span with [`TimeSpan::from_secs`], `from_mins`, `from_hours` or
//! `from_days`, which are `const`, and read one with [`TimeSpan::as_secs`].
//! Both types are `Copy`.
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
//! let now = origin + TimeSpan::from_secs(150);
//!
//! assert_eq!(
//!     period_at(origin, EPOCH, now),
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
//! # Encoding
//!
//! Both types encode as one `u64`, a count of nanoseconds, and the contract
//! spec shows them as `u64`. Changing a field that holds nanoseconds as a
//! `u64` to either type changes neither a storage layout nor an ABI.
//!
//! # Host time
//!
//! [`core::time::Duration`] has no XDR form, so it cannot be stored or
//! passed to a contract. Convert at the edge of host code with
//! [`TimeSpan::from_duration`] and `Duration::from`.

use crate::types::{TimePoint, TimeSpan};

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
    origin: TimePoint,
    period: TimeSpan,
    at: TimePoint,
) -> Option<Period> {
    // The quotient and remainder of two spans, on their shared unit.
    let since = *at.checked_duration_since(&origin)?;
    let period = *period;
    if period == 0 {
        return None;
    }
    Some(Period {
        index: since / period,
        elapsed: TimeSpan::new(since % period),
    })
}
