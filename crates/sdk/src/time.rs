//! Time in contracts: [`core::time::Duration`] and the unit constants
//! Solidity spells `hours`, `days` and `weeks`.
//!
//! [`TimePoint`], the block timepoint from
//! [`get_block_timepoint`](crate::block::get_block_timepoint), does its own
//! arithmetic with `Duration` -- `+`, `-`, `+=`, `-=`, `TimePoint - TimePoint`,
//! and the checked and saturating forms `std::time::Instant` offers, such as
//! [`TimePoint::checked_add`] and [`TimePoint::duration_since`]. This module
//! only adds the constants, since `Duration::from_days` is not yet stable:
//!
//! ```
//! use sdk::TimePoint;
//! use sdk::time::{DAY, Duration, WEEK};
//!
//! let now = TimePoint::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
//! let deadline = &now + 7 * DAY;
//!
//! assert!(now < deadline);
//! assert_eq!(&deadline - &now, WEEK);
//! assert_eq!(now.duration_since(&deadline), Duration::ZERO);
//! ```
//!
//! [`BlockHeight`](crate::BlockHeight) has the same shape against a signed
//! number of blocks: `height + 100`, `later - earlier`,
//! [`BlockHeight::checked_add`](crate::BlockHeight::checked_add) and
//! [`BlockHeight::blocks_since`](crate::BlockHeight::blocks_since).
//!
//! # Overflow
//!
//! Unlike the integer primitives, this arithmetic checks in every profile:
//! an operator that overflows, or a timepoint subtraction that would go
//! before the Unix epoch, panics whether or not `overflow-checks` is on --
//! the same rule as `core::time`. A deadline that silently wrapped into the
//! past would be a security bug, so a native test that passes says the same
//! thing about the deployed contract.
//!
//! `TimePoint - TimePoint` saturates to [`Duration::ZERO`] when the
//! right-hand side is later, as `std::time::Instant` does; use
//! [`TimePoint::checked_duration_since`] to detect that case.
//!
//! # Storing a span
//!
//! `Duration` has no wire encoding, so it cannot be a `#[contract_type]`
//! field or a method argument. Store the endpoints instead -- `TimePoint`
//! and `BlockHeight` both encode -- or carry the span as `u64` nanoseconds
//! through [`Duration::as_nanos`] and [`Duration::from_nanos`].

pub use core::time::Duration;

use crate::types::TimePoint;

/// One millisecond.
pub const MILLISECOND: Duration = Duration::from_millis(1);
/// One second.
pub const SECOND: Duration = Duration::from_secs(1);
/// One minute.
pub const MINUTE: Duration = Duration::from_mins(1);
/// One hour.
pub const HOUR: Duration = Duration::from_hours(1);
/// One day of 24 hours.
pub const DAY: Duration = Duration::from_hours(24);
/// One week of 7 days.
pub const WEEK: Duration = Duration::from_hours(7 * 24);

// The doc example above is the test; this just keeps `TimePoint` in scope
// for the intra-doc links when the crate is built without it.
const _: fn() -> TimePoint = || TimePoint::UNIX_EPOCH;
