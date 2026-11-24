use super::keypath::{Keypath, LeafKey};
use super::value::Value;
use crate::exn::LibraryExn;
use crate::re_throw::OrReThrow;
use crate::sys_exn::nnint_internal as nnint;
use core::time::Duration;

/// A mergeable integer whose contents belong to a period, and start over
/// when the period changes. **Prototype**: see [the verdict](#verdict).
///
/// A rate-limit window, a per-epoch tally and a token bucket are all this
/// cell. Say when it starts over and what to, as a [`Cycle`], and use it
/// like a counter:
///
/// ```ignore
/// // At most `limit` units an hour.
/// let quota = Period::hourly().reset_to(limit);
/// ensure(self.spent.take(quota, amount), Error::OverLimit);
/// let left = self.spent.get(quota);
///
/// // A tally per epoch, per pixel.
/// let epoch = Period::every(EPOCH).reset_to(0);
/// self.strokes.at_mut(&pixel).add(epoch, 1);
///
/// // A carry-over: half of the last period's tally survives each period.
/// let fading = Period::daily().reset_with(|was| was.value >> was.elapsed().min(127));
/// self.heat.add(fading, 1);
/// ```
///
/// # Why it is a type
///
/// The first touch in a new period must replace what the last period left
/// before adding to it. On Thruput "the first touch" is every transaction
/// in the first block of the period. They all execute against the same
/// snapshot, and they can all commit only if each writes the *same* reset
/// and then its own delta: identical sets of an integer merge, and so do
/// the deltas on top of them (see [`NNInt`](super::NNInt)).
///
/// `Rollover` owns what a hand-written version has to get right:
///
/// - **The reset is computed from the snapshot.** A rule sees the value and
///   period as of the block's start ([`Lapsed`]), never this transaction's
///   own pending changes.
/// - **The reset is the first write, once.** A second touch in the same
///   transaction resets nothing. A reset that would discard writes this
///   transaction has already made is refused with
///   [`LibraryExn::RolloverAlreadyWritten`].
/// - **Every write resets first.** There is no way to add to a lapsed cell.
/// - **Readers see a lapsed cell as reset**, without writing, by passing
///   [`get`](Self::get) the cycle the writers use.
///
/// The value is in `0..=i128::MAX`. Like [`NNInt::inc`](super::NNInt::inc),
/// [`add`](Self::add) and [`sub`](Self::sub) are checked at *commit*, for
/// the block's merged total: a block's transactions that together take the
/// value below zero are not all admitted. That is what makes
/// [`take`](Self::take) a rate limit: it refuses what this transaction can
/// see does not fit, and the runtime refuses what the block cannot fit.
///
/// # What the contract still owes
///
/// Every transaction in a block must compute the same reset. A cycle must
/// therefore depend only on the [`Lapsed`] value and on inputs every
/// transaction in the block sees alike: the block's timepoint and height,
/// and contract state *as of the block's start*. `Period`'s constructors
/// read the block; `Period::from_raw` is the escape hatch.
///
/// **Known hazard.** If a transaction changes an input to the reset (a
/// limit, say) and then touches the cell, it computes a different reset
/// from the rest of the block. Whichever side commits second is left out
/// with a `ValueMismatch` conflict. Nothing is corrupted, but transactions
/// that did nothing wrong can be the ones delayed. Change a cycle's inputs
/// in a transaction that does not touch the cell.
///
/// The reset is consulted only when the period changes. A limit lowered
/// mid-period applies from the next period.
///
/// # Storage
///
/// Two storage items: the period in one key and the integer in another. A
/// reset writes both. A touch that does not reset reads the period and
/// writes one delta, and a constant reset never loads the old value.
/// Several operations in one call are cheaper through
/// [`enter`](Self::enter), which checks the period once.
///
/// # What it does not express
///
/// - The value is one integer. Several counters packed side by side in it
///   still merge (each delta adds lane-wise), but the lane layout and its
///   carry analysis remain the contract's.
/// - A reset that depends on other transactions in the same block. No
///   contract can express that; they are not visible.
/// - An upper bound below `i128::MAX`. The runtime enforces only `0` and
///   the integer's own range across a block. Count *down* from a limit
///   rather than up toward one.
///
/// # Verdict
///
/// This is a prototype to judge the idea by. Its tests are in
/// `crates/sdk/tests/rollover.rs`, and include a token bucket and a
/// lane-packed tally written against it.
///
/// For it: one type covers the constant reset and the carry-over. Adding
/// without resetting, and resetting twice in a transaction, stop being
/// possible. Readers and writers share one [`Cycle`] value.
///
/// Against it: the logic it replaces is about ten lines per contract. It
/// cannot check the property that matters most, that a cycle's inputs are
/// the same across the block. Nor can it check the rule: a refill that
/// rounds differently on each call is as easy to write against this type as
/// without it. A value kept in a [`Value`], where every transaction in the
/// block writes the same whole state and none adds to it, needs none of
/// this.
///
/// It has not been built into a guest contract, measured for gas, or
/// added to the fixtures.
#[derive(Debug)]
pub struct Rollover {
    period: Value<u64>,
    cell: LeafKey,
}

/// A period: the index a [`Rollover`] cell marks its contents with.
///
/// The constructors derive it from the block, which is what keeps it
/// identical across the block's transactions. Turn one into a [`Cycle`]
/// with [`reset_to`](Self::reset_to) or [`reset_with`](Self::reset_with).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Period(u64);

const HOUR: Duration = Duration::from_secs(3_600);

impl Period {
    /// Fixed windows of `length`, counted from the Unix epoch: the window
    /// the block's timepoint falls in.
    ///
    /// `length` is taken in whole nanoseconds, and as one if it is less. It
    /// must not change while a cell is in use: see the hazard on
    /// [`Rollover`].
    pub fn every(length: Duration) -> Self {
        Self::every_from(0, length)
    }

    /// Fixed windows of `length`, counted from `origin` (Unix nanoseconds)
    /// instead of the Unix epoch. A block before `origin` is in window 0.
    pub fn every_from(origin: u64, length: Duration) -> Self {
        let now = crate::block::get_block_timepoint().into_inner();
        let length = u64::try_from(length.as_nanos()).unwrap_or(u64::MAX);
        Self(now.saturating_sub(origin) / length.max(1))
    }

    /// The hour the block's timepoint falls in.
    pub fn hourly() -> Self {
        Self::every(HOUR)
    }

    /// The UTC day the block's timepoint falls in.
    pub fn daily() -> Self {
        Self::every(HOUR * 24)
    }

    /// The block's height: a new period every block.
    pub fn block() -> Self {
        Self(crate::block::get_block_height().into_inner() as u64)
    }

    /// The block's timepoint, in Unix nanoseconds: a new period every
    /// block, marked with its time. For rules that measure elapsed time.
    pub fn timepoint() -> Self {
        Self(crate::block::get_block_timepoint().into_inner())
    }

    /// A period the contract computed itself. It must be the same for every
    /// transaction in a block, and never go back.
    pub const fn from_raw(index: u64) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u64 {
        self.0
    }

    /// A cycle that starts each period at `value`.
    pub const fn reset_to(self, value: i128) -> Cycle<i128> {
        Cycle {
            period: self,
            reset: value,
        }
    }

    /// A cycle that starts each period at what `rule` makes of the last
    /// one's contents.
    ///
    /// `rule` runs only when the cell's contents are from another period.
    /// It may return an `i128`, or an `Option<i128>` where `None` leaves
    /// the cell in its old period: nothing is written, and the next touch
    /// asks again. Return `None` when the period has changed but the value
    /// has not, to save the write, provided the rule gives the same total
    /// whether or not the intermediate step is recorded.
    ///
    /// See [`Rollover`] for what `rule` may depend on.
    pub const fn reset_with<F, O>(self, rule: F) -> Cycle<F>
    where
        F: Fn(Lapsed) -> O,
        O: Into<Option<i128>>,
    {
        Cycle {
            period: self,
            reset: rule,
        }
    }
}

/// When a [`Rollover`] cell starts over, and what to. Made by
/// [`Period::reset_to`] and [`Period::reset_with`].
///
/// Give readers and writers of a cell the same cycle. It is `Copy` when
/// its rule is, as a constant or a closure that borrows is.
#[derive(Clone, Copy, Debug)]
pub struct Cycle<R> {
    period: Period,
    reset: R,
}

impl<R> Cycle<R> {
    pub const fn period(&self) -> Period {
        self.period
    }
}

/// What a [`Cycle`] resets a cell to: a constant, or a function of the
/// [`Lapsed`] contents.
pub trait Reset {
    /// Whether [`reset`](Self::reset) reads [`Lapsed::value`]. A reset that
    /// does not spares the load.
    #[doc(hidden)]
    const READS_VALUE: bool = true;

    /// The value the new period starts at, or `None` to stay in the old
    /// one.
    fn reset(&self, lapsed: Lapsed) -> Option<i128>;
}

impl Reset for i128 {
    const READS_VALUE: bool = false;

    fn reset(&self, _: Lapsed) -> Option<i128> {
        Some(*self)
    }
}

impl<F, O> Reset for F
where
    F: Fn(Lapsed) -> O,
    O: Into<Option<i128>>,
{
    fn reset(&self, lapsed: Lapsed) -> Option<i128> {
        self(lapsed).into()
    }
}

/// What a reset rule is given: a cell whose period has passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lapsed {
    /// The cell's value as of the block's start.
    pub value: i128,

    /// The period that value is from, or `None` if the cell was never
    /// written.
    pub from: Option<u64>,

    /// The period being entered.
    pub to: u64,
}

impl Lapsed {
    /// How many periods have passed since the value was current. Zero for a
    /// cell never written.
    pub fn elapsed(&self) -> u64 {
        self.from.map_or(0, |from| self.to.saturating_sub(from))
    }
}

impl From<Keypath> for Rollover {
    fn from(keypath: Keypath) -> Self {
        Self {
            period: keypath.child(&0u32).into(),
            cell: keypath.child(&1u32).into(),
        }
    }
}

fn sum(set: i128, add: i128) -> Result<i128, LibraryExn> {
    set.checked_add(add)
        .ok_or(LibraryExn::NonNegativeIntOverflow)
}

impl Rollover {
    /// The value in this period, without writing: what the cell holds if
    /// its contents are current, and otherwise what `cycle` would reset it
    /// to. Includes what this transaction has added.
    pub fn get(&self, cycle: Cycle<impl Reset>) -> i128 {
        self.try_get(cycle).or_re_throw()
    }

    /// Like [`get`](Self::get), but report failure as `Err` instead of
    /// aborting (the `try_` law; see the [`exn`](mod@crate::exn) module
    /// docs).
    pub fn try_get<R: Reset>(
        &self,
        cycle: Cycle<R>,
    ) -> Result<i128, LibraryExn> {
        let recorded = self.recorded_period()?;
        let (set, add) = self.load()?;
        if recorded != Some(cycle.period.0) {
            let lapsed = Lapsed {
                value: set,
                from: recorded,
                to: cycle.period.0,
            };
            if let Some(value) = cycle.reset.reset(lapsed) {
                return Ok(value);
            }
        }
        sum(set, add)
    }

    /// Add `delta`, which may be negative, starting the period over first
    /// if it is new. The range is enforced at commit, for the block's
    /// merged total.
    ///
    /// *Throws:* any error [`try_enter`](Self::try_enter) or
    /// [`Current::try_add`] would return.
    pub fn add(&mut self, cycle: Cycle<impl Reset>, delta: i128) {
        self.enter(cycle).add(delta)
    }

    /// Subtract `amount`: [`add`](Self::add) of its negation.
    pub fn sub(&mut self, cycle: Cycle<impl Reset>, amount: i128) {
        self.enter(cycle).sub(amount)
    }

    /// Subtract `amount` if the cell holds that much, and say whether it
    /// did. See [`Current::take`].
    #[must_use = "a refused take subtracts nothing"]
    pub fn take(&mut self, cycle: Cycle<impl Reset>, amount: i128) -> bool {
        self.enter(cycle).take(amount)
    }

    /// Start the period over if it is new, and return a guard for several
    /// operations in it. [`add`](Self::add), [`sub`](Self::sub) and
    /// [`take`](Self::take) are this and one operation.
    ///
    /// *Throws:* any error [`try_enter`](Self::try_enter) would return.
    pub fn enter(&mut self, cycle: Cycle<impl Reset>) -> Current<'_> {
        self.try_enter(cycle).or_re_throw()
    }

    /// Like [`enter`](Self::enter), but report failure as `Err` instead of
    /// aborting.
    ///
    /// [`LibraryExn::RolloverAlreadyWritten`] if the reset would discard
    /// writes this transaction has already made to the cell: its period, or
    /// what its cycle resets to, changed within the transaction. A negative
    /// reset value is an underflow. On `Err` the cell is unmodified.
    pub fn try_enter<R: Reset>(
        &mut self,
        cycle: Cycle<R>,
    ) -> Result<Current<'_>, LibraryExn> {
        let Cycle { period, reset } = cycle;
        let recorded = self.recorded_period()?;
        if recorded == Some(period.0) {
            return Ok(Current { cell: self });
        }
        // Lapsed, or never written. What this transaction has pending tells
        // whether it has been here before, without a load.
        let (set, add) = nnint::try_get_values(self.cell.get())?;
        if set.is_some() {
            // Reset already, under another period.
            return Err(LibraryExn::RolloverAlreadyWritten);
        }
        // No pending set, so the base the rule sees is the snapshot.
        let value = if R::READS_VALUE { self.load()?.0 } else { 0 };
        let lapsed = Lapsed {
            value,
            from: recorded,
            to: period.0,
        };
        if let Some(value) = reset.reset(lapsed) {
            if add != 0 {
                // An earlier touch was told there was nothing to reset.
                return Err(LibraryExn::RolloverAlreadyWritten);
            }
            self.write(period, value)?;
        }
        Ok(Current { cell: self })
    }

    /// Set the cell to `value` for `period`, whatever it held.
    ///
    /// This is not a rollover: transactions in the same block that touch
    /// the cell compute a different reset and conflict with it. Use it to
    /// lay the cell out, or when the terms of its cycle change.
    ///
    /// *Throws:* [`LibraryExn::RolloverAlreadyWritten`] if this transaction
    /// has added to the cell; a negative `value` as an underflow.
    pub fn restart(&mut self, period: Period, value: i128) {
        self.try_restart(period, value).or_re_throw()
    }

    /// Like [`restart`](Self::restart), but report failure as `Err` instead
    /// of aborting.
    pub fn try_restart(
        &mut self,
        period: Period,
        value: i128,
    ) -> Result<(), LibraryExn> {
        let (_, add) = nnint::try_get_values(self.cell.get())?;
        if add != 0 {
            return Err(LibraryExn::RolloverAlreadyWritten);
        }
        self.write(period, value)
    }

    /// What is stored, whatever the period: the period it is from (`None`
    /// if never written) and the value. For tests and diagnostics.
    pub fn recorded(&self) -> (Option<u64>, i128) {
        let (set, add) = self.load().or_re_throw();
        (
            self.recorded_period().or_re_throw(),
            sum(set, add).or_re_throw(),
        )
    }

    /// The stored period as this transaction sees it.
    fn recorded_period(&self) -> Result<Option<u64>, LibraryExn> {
        match self.period.try_get() {
            Ok(period) => Ok(Some(period)),
            Err(e) if e.is_storage_missing() => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The integer's base and delta, reading a never-written cell as zero.
    /// The base is the pending set, or failing that the snapshot.
    fn load(&self) -> Result<(i128, i128), LibraryExn> {
        match nnint::try_get_values_force(self.cell.get()) {
            Ok(values) => Ok(values),
            Err(e) if e.is_storage_missing() => Ok((0, 0)),
            Err(e) => Err(e),
        }
    }

    /// Set `value` for `period`, discarding pending deltas.
    fn write(
        &mut self,
        period: Period,
        value: i128,
    ) -> Result<(), LibraryExn> {
        if value < 0 {
            return Err(LibraryExn::NonNegativeIntUnderflow);
        }
        self.period.try_set(&period.0)?;
        let key = &self.cell;
        nnint::try_set_add(key.get(), Some(value), 0, key.is_exclusive())
    }
}

/// A [`Rollover`] cell brought into the current period by
/// [`Rollover::enter`], for several operations at the cost of one check.
pub struct Current<'a> {
    cell: &'a mut Rollover,
}

impl Current<'_> {
    /// The value: as of the block's start, reset if the period changed,
    /// plus what this transaction has added.
    ///
    /// Other transactions in the block are not visible. Use it to refuse
    /// what could never fit; whether the block's deltas fit together is
    /// checked at commit.
    pub fn get(&self) -> i128 {
        self.try_get().or_re_throw()
    }

    /// Like [`get`](Self::get), but report failure as `Err` instead of
    /// aborting.
    pub fn try_get(&self) -> Result<i128, LibraryExn> {
        let (set, add) = self.cell.load()?;
        sum(set, add)
    }

    /// Add `delta`, which may be negative, without loading the value.
    ///
    /// The range is enforced at commit, for the block's merged total.
    /// Throws only if the deltas accumulated this transaction overflow.
    pub fn add(&mut self, delta: i128) {
        self.try_add(delta).or_re_throw()
    }

    /// Like [`add`](Self::add), but report failure as `Err` instead of
    /// aborting.
    pub fn try_add(&mut self, delta: i128) -> Result<(), LibraryExn> {
        let key = &self.cell.cell;
        let (set, add) = nnint::try_get_values(key.get())?;
        let add = add.checked_add(delta).ok_or(if delta < 0 {
            LibraryExn::NonNegativeIntUnderflow
        } else {
            LibraryExn::NonNegativeIntOverflow
        })?;
        nnint::try_set_add(key.get(), set, add, key.is_exclusive())
    }

    /// Subtract `amount`: [`add`](Self::add) of its negation.
    pub fn sub(&mut self, amount: i128) {
        let delta = amount
            .checked_neg()
            .ok_or(LibraryExn::NonNegativeIntOverflow)
            .or_re_throw();
        self.add(delta)
    }

    /// Subtract `amount` if the cell holds that much, and say whether it
    /// did.
    ///
    /// The check is against what this transaction can see, which gives a
    /// take that could never fit a clear answer. Whether the takes in a
    /// block fit together is the runtime's check, at commit.
    #[must_use = "a refused take subtracts nothing"]
    pub fn take(&mut self, amount: i128) -> bool {
        if self.get() < amount {
            return false;
        }
        self.sub(amount);
        true
    }
}

impl super::sealed::Sealed for Rollover {}
impl super::node::StorageType for Rollover {}

impl super::deletable::Deletable for Rollover {
    fn delete_internal(&mut self, exclusive: bool) {
        self.period.delete_internal(exclusive);
        let exclusive = exclusive || self.cell.is_exclusive();
        crate::sys_exn::delete::delete(self.cell.get(), exclusive);
    }
}
