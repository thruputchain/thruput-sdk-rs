use super::keypath::{Keypath, LeafKey};
use super::value::Value;
use crate::exn::LibraryExn;
use crate::re_throw::OrReThrow;
use crate::sys_exn::nnint_internal as nnint;
use core::num::NonZeroU64;

/// A mergeable integer whose contents belong to a period, and start over
/// when the period changes. **Prototype**: see [the verdict](#verdict).
///
/// A rate-limit window, a per-epoch tally and a token bucket are all this
/// cell: the first touch in a new period must replace what the last period
/// left before adding to it. On Thruput "the first touch" is every
/// transaction in the first block of the period. They all execute against
/// the same snapshot, and they can all commit only if each writes the *same*
/// reset and then its own delta: identical sets of an integer merge, and so
/// do the deltas on top of them (see [`NNInt`](super::NNInt)).
///
/// `Rollover` owns the four things a hand-written version has to get right:
///
/// - **The reset is computed from the snapshot.** The rule sees the value
///   and tag as of the block's start ([`Lapsed`]) and the new [`Period`],
///   never this transaction's own pending changes.
/// - **The reset is the first write, once.** A second
///   [`enter`](Self::enter) in the same transaction finds the new tag and
///   resets nothing. If a reset would discard writes this transaction has
///   already made, `enter` reports [`LibraryExn::RolloverAlreadyWritten`]
///   instead.
/// - **The tag advances with the reset**, in the same call.
/// - **Readers see a lapsed cell as reset**, without writing:
///   [`peek`](Self::peek).
///
/// Adding is only possible through the [`Current`] guard that `enter`
/// returns, so a delta cannot land on a lapsed cell.
///
/// ```ignore
/// // A fixed window: at most `limit` units per hour.
/// let hour = Period::window(origin, HOUR);
/// let mut left = self.quota.enter(hour, limit);
/// ensure(left.get() >= amount, Error::OverLimit);
/// left.add(-amount);
///
/// // A carry-over: what is left of last period's tally, halved per period.
/// let mut tally = self.tally.enter_with(hour, |l| {
///     Some(l.value >> (l.to - l.from.unwrap_or(l.to)).min(127))
/// });
/// tally.add(1);
/// ```
///
/// The value is in `0..=L::MAX`. Like [`NNInt::inc`](super::NNInt::inc),
/// [`Current::add`] is checked at *commit*, for the block's merged total: a
/// block's transactions that together take the value below zero, or above
/// the maximum, are not all admitted.
///
/// # What the contract still owes
///
/// Every transaction in a block must compute the same reset. The rule and
/// the period must therefore depend only on the [`Lapsed`] value and on
/// inputs every transaction in the block sees alike: the block's timepoint
/// and height, and contract state *as of the block's start*. `Period`'s
/// constructors read the block; `Period::from_raw` is the escape hatch.
///
/// **Known hazard.** If a transaction changes an input to the rule (a
/// limit, say) and then enters the cell, it computes a different reset from
/// the rest of the block. Whichever side commits second is left out with a
/// `ValueMismatch` conflict. Nothing is corrupted, but transactions that
/// did nothing wrong can be the ones delayed. Change a rule's inputs in a
/// transaction that does not enter the cell.
///
/// The reset value is consulted only when the period changes. A limit
/// lowered mid-period applies from the next period.
///
/// # Where the tag lives
///
/// | Layout | Storage items | Writes on reset | Value range | Tag |
/// | --- | --- | --- | --- | --- |
/// | [`Split`] (default) | 2 | 2 | `0..=i128::MAX` | 64 bits, own key |
/// | [`Packed`] | 1 | 1 | `0..2^95` | 32 bits, modulo `2^32 - 1` |
///
/// A touch that does not reset reads the tag and writes one delta in either
/// layout. With `Split` a constant reset never loads the old value.
/// `Packed` is what a contract with thousands of cells wants (one deposit
/// per cell); it gives up 32 bits of value, and a cell left untouched for
/// exactly `2^32 - 1` periods is taken for current.
///
/// # What it does not express
///
/// - The value is one integer. Several counters packed side by side in it
///   still merge (each delta adds lane-wise), but the lane layout and its
///   carry analysis remain the contract's.
/// - A reset that depends on other transactions in the same block. No
///   contract can express that; they are not visible.
/// - An upper bound below the layout's maximum. The runtime enforces only
///   `0` and the integer's own range across a block. Count *down* from a
///   limit rather than up toward one.
///
/// # Verdict
///
/// This is a prototype to judge the idea by. Its tests are in
/// `crates/sdk/tests/rollover.rs`, and include a token bucket and a
/// lane-packed tally written against it.
///
/// For it: one type covers the constant reset and the carry-over, in both
/// layouts. It turns two conventions into things that cannot be got
/// wrong: adding without resetting, and resetting twice in a transaction.
/// Readers get the writers' rule through [`peek_with`](Self::peek_with).
///
/// Against it: the logic it replaces is about ten lines per contract. It
/// cannot check the property that matters most, that the rule's inputs are
/// the same across the block. Nor can it check the rule: a refill that
/// rounds differently on each call is as easy to write against this type as
/// without it. A value kept in a [`Value`], where every transaction in the
/// block writes the same whole state and none adds to it, needs none of
/// this.
///
/// It has not been built into a guest contract, measured for gas, or
/// added to the fixtures.
#[derive(Debug)]
pub struct Rollover<L: TagLayout = Split> {
    keys: L::Keys,
}

/// A period: the index a [`Rollover`] cell tags its contents with.
///
/// The constructors derive it from the block, which is what keeps it
/// identical across the block's transactions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Period(u64);

impl Period {
    /// The block's height: a new period every block.
    pub fn block_height() -> Self {
        Self(crate::block::get_block_height().into_inner() as u64)
    }

    /// The block's timepoint, in Unix nanoseconds: a new period every
    /// block, tagged with its time. For rules that measure elapsed time.
    pub fn timepoint() -> Self {
        Self(crate::block::get_block_timepoint().into_inner())
    }

    /// The fixed window of `length` nanoseconds that the block's timepoint
    /// falls in, counting windows from `origin`. A block before `origin` is
    /// in window 0.
    ///
    /// `origin` and `length` must not change while the cell is in use: see
    /// the hazard on [`Rollover`].
    pub fn window(origin: u64, length: NonZeroU64) -> Self {
        let now = crate::block::get_block_timepoint().into_inner();
        Self(now.saturating_sub(origin) / length)
    }

    /// A period the contract computed itself. It must be the same for every
    /// transaction in a block, and never go back.
    pub const fn from_raw(index: u64) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u64 {
        self.0
    }
}

/// What a reset rule is given: a cell whose period has passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lapsed {
    /// The cell's value as of the block's start.
    pub value: i128,

    /// The period that value is from, or `None` if the cell was never
    /// written. [`Packed`] keeps the period modulo `2^32 - 1`.
    pub from: Option<u64>,

    /// The period being entered.
    pub to: u64,
}

/// Where a [`Rollover`] keeps its period tag. Implemented by [`Split`] and
/// [`Packed`].
#[allow(private_bounds)]
pub trait TagLayout: Layout {}

/// The tag in its own key: two storage items, the full integer range.
#[derive(Debug)]
pub struct Split;

/// The tag in the low 32 bits of the integer: one storage item, 95 bits of
/// value.
///
/// The tag is the period modulo `2^32 - 1`, plus one, so that a zero tag
/// means the cell was never written.
#[derive(Debug)]
pub struct Packed;

impl TagLayout for Split {}
impl TagLayout for Packed {}

/// A cell's base and delta, in the value's own units: `set` is the pending
/// set, or failing that the snapshot.
struct Loaded {
    tag: Option<u64>,
    set: i128,
    add: i128,
}

trait Layout {
    type Keys: From<Keypath> + core::fmt::Debug;
    const MAX: i128;

    /// The tag, as this transaction sees it.
    fn tag(keys: &Self::Keys) -> Result<Option<u64>, LibraryExn>;
    /// Tag and value, as this transaction sees them.
    fn load(keys: &Self::Keys) -> Result<Loaded, LibraryExn>;
    /// The integer's key, for its pending values.
    fn cell(keys: &Self::Keys) -> &LeafKey;
    fn same(tag: u64, period: Period) -> bool;
    /// Set `value` for `period`, discarding pending deltas.
    fn reset(
        keys: &mut Self::Keys,
        period: Period,
        value: i128,
    ) -> Result<(), LibraryExn>;
    /// `delta`, in the units the integer is stored in.
    fn scale(delta: i128) -> Result<i128, LibraryExn>;
    fn delete(keys: &mut Self::Keys, exclusive: bool);
}

fn sum(set: i128, add: i128) -> Result<i128, LibraryExn> {
    set.checked_add(add)
        .ok_or(LibraryExn::NonNegativeIntOverflow)
}

/// The integer's base and delta, reading a never-written cell as zero.
fn load_cell(key: &LeafKey) -> Result<(i128, i128), LibraryExn> {
    match nnint::try_get_values_force(key.get()) {
        Ok(values) => Ok(values),
        Err(e) if e.is_storage_missing() => Ok((0, 0)),
        Err(e) => Err(e),
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct SplitKeys {
    tag: Value<u64>,
    cell: LeafKey,
}

impl From<Keypath> for SplitKeys {
    fn from(keypath: Keypath) -> Self {
        Self {
            tag: keypath.child(&0u32).into(),
            cell: keypath.child(&1u32).into(),
        }
    }
}

impl Layout for Split {
    type Keys = SplitKeys;
    const MAX: i128 = i128::MAX;

    fn tag(keys: &SplitKeys) -> Result<Option<u64>, LibraryExn> {
        match keys.tag.try_get() {
            Ok(tag) => Ok(Some(tag)),
            Err(e) if e.is_storage_missing() => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn load(keys: &SplitKeys) -> Result<Loaded, LibraryExn> {
        let (set, add) = load_cell(&keys.cell)?;
        Ok(Loaded {
            tag: Self::tag(keys)?,
            set,
            add,
        })
    }

    fn cell(keys: &SplitKeys) -> &LeafKey {
        &keys.cell
    }

    fn same(tag: u64, period: Period) -> bool {
        tag == period.0
    }

    fn reset(
        keys: &mut SplitKeys,
        period: Period,
        value: i128,
    ) -> Result<(), LibraryExn> {
        keys.tag.try_set(&period.0)?;
        let key = &keys.cell;
        nnint::try_set_add(key.get(), Some(value), 0, key.is_exclusive())
    }

    fn scale(delta: i128) -> Result<i128, LibraryExn> {
        Ok(delta)
    }

    fn delete(keys: &mut SplitKeys, exclusive: bool) {
        use super::deletable::Deletable;
        keys.tag.delete_internal(exclusive);
        let exclusive = exclusive || keys.cell.is_exclusive();
        crate::sys_exn::delete::delete(keys.cell.get(), exclusive);
    }
}

const TAG_BITS: u32 = 32;

/// A period's packed tag: never zero, which is a never-written cell's.
fn packed_tag(period: Period) -> u32 {
    (period.0 % u64::from(u32::MAX)) as u32 + 1
}

impl Layout for Packed {
    type Keys = LeafKey;
    const MAX: i128 = i128::MAX >> TAG_BITS;

    fn tag(key: &LeafKey) -> Result<Option<u64>, LibraryExn> {
        Ok(Self::load(key)?.tag)
    }

    fn load(key: &LeafKey) -> Result<Loaded, LibraryExn> {
        // Deltas are multiples of `2^TAG_BITS`, so the tag is the base's.
        let (set, add) = load_cell(key)?;
        Ok(Loaded {
            tag: (set as u32).checked_sub(1).map(u64::from),
            set: set >> TAG_BITS,
            add: add >> TAG_BITS,
        })
    }

    fn cell(key: &LeafKey) -> &LeafKey {
        key
    }

    fn same(tag: u64, period: Period) -> bool {
        tag + 1 == u64::from(packed_tag(period))
    }

    fn reset(
        key: &mut LeafKey,
        period: Period,
        value: i128,
    ) -> Result<(), LibraryExn> {
        let packed = (value << TAG_BITS) | i128::from(packed_tag(period));
        nnint::try_set_add(key.get(), Some(packed), 0, key.is_exclusive())
    }

    fn scale(delta: i128) -> Result<i128, LibraryExn> {
        delta.checked_mul(1 << TAG_BITS).ok_or(if delta < 0 {
            LibraryExn::NonNegativeIntUnderflow
        } else {
            LibraryExn::NonNegativeIntOverflow
        })
    }

    fn delete(key: &mut LeafKey, exclusive: bool) {
        let exclusive = exclusive || key.is_exclusive();
        crate::sys_exn::delete::delete(key.get(), exclusive);
    }
}

impl<L: TagLayout> From<Keypath> for Rollover<L> {
    fn from(keypath: Keypath) -> Self {
        Self {
            keys: keypath.into(),
        }
    }
}

fn in_range<L: Layout>(value: i128) -> Result<i128, LibraryExn> {
    if value < 0 {
        Err(LibraryExn::NonNegativeIntUnderflow)
    } else if value > L::MAX {
        Err(LibraryExn::NonNegativeIntOverflow)
    } else {
        Ok(value)
    }
}

impl<L: TagLayout> Rollover<L> {
    /// The largest value the cell holds.
    pub const MAX: i128 = L::MAX;

    /// Enter `period`, resetting the cell to `reset_to` if its contents are
    /// from another period, and return a guard to read and add through.
    ///
    /// *Throws:* any error [`try_enter`](Self::try_enter) would return.
    pub fn enter(&mut self, period: Period, reset_to: i128) -> Current<'_, L> {
        self.try_enter(period, reset_to).or_re_throw()
    }

    /// Like [`enter`](Self::enter), but report failure as `Err` instead of
    /// aborting (the `try_` law; see the [`exn`](mod@crate::exn) module
    /// docs).
    ///
    /// [`LibraryExn::RolloverAlreadyWritten`] if this transaction has
    /// already written the cell under a different period, so that the reset
    /// would discard those writes. A `reset_to` outside `0..=MAX` is an
    /// underflow or overflow. On `Err` the cell is unmodified.
    pub fn try_enter(
        &mut self,
        period: Period,
        reset_to: i128,
    ) -> Result<Current<'_, L>, LibraryExn> {
        self.roll(period, |_, _| Ok(Some(reset_to)))?;
        Ok(Current { cell: self })
    }

    /// Enter `period`, resetting the cell to what `rule` makes of its
    /// lapsed contents.
    ///
    /// `rule` runs only if the cell's contents are from another period. It
    /// is given the value as of the block's start, never this transaction's
    /// own changes. `None` leaves the cell in its old period: nothing is
    /// written, and the next touch asks again. Return it when the period
    /// has changed but the value has not, to save the write, provided the
    /// rule gives the same total whether or not the intermediate step is
    /// recorded.
    ///
    /// See [`Rollover`] for what `rule` may depend on.
    ///
    /// *Throws:* any error [`try_enter_with`](Self::try_enter_with) would
    /// return.
    pub fn enter_with(
        &mut self,
        period: Period,
        rule: impl FnOnce(Lapsed) -> Option<i128>,
    ) -> Current<'_, L> {
        self.try_enter_with(period, rule).or_re_throw()
    }

    /// Like [`enter_with`](Self::enter_with), but report failure as `Err`
    /// instead of aborting. The error contract is
    /// [`try_enter`](Self::try_enter)'s.
    pub fn try_enter_with(
        &mut self,
        period: Period,
        rule: impl FnOnce(Lapsed) -> Option<i128>,
    ) -> Result<Current<'_, L>, LibraryExn> {
        self.roll(period, |keys, from| {
            let value = L::load(keys)?.set;
            Ok(rule(Lapsed {
                value,
                from,
                to: period.0,
            }))
        })?;
        Ok(Current { cell: self })
    }

    fn roll(
        &mut self,
        period: Period,
        reset: impl FnOnce(
            &L::Keys,
            Option<u64>,
        ) -> Result<Option<i128>, LibraryExn>,
    ) -> Result<(), LibraryExn> {
        let tag = L::tag(&self.keys)?;
        if tag.is_some_and(|tag| L::same(tag, period)) {
            return Ok(());
        }
        // Lapsed, or never written. What this transaction has pending tells
        // whether it has been here before, without a load.
        let (set, add) = nnint::try_get_values(L::cell(&self.keys).get())?;
        if set.is_some() {
            // Reset already, under another period.
            return Err(LibraryExn::RolloverAlreadyWritten);
        }
        // No pending set, so the base the rule sees is the snapshot.
        let Some(value) = reset(&self.keys, tag)? else {
            return Ok(());
        };
        if add != 0 {
            // An earlier touch was told there was nothing to reset.
            return Err(LibraryExn::RolloverAlreadyWritten);
        }
        L::reset(&mut self.keys, period, in_range::<L>(value)?)
    }

    /// The cell's value in `period`, without writing: what it holds if its
    /// contents are from `period`, and otherwise `reset_to`.
    pub fn peek(&self, period: Period, reset_to: i128) -> i128 {
        self.try_peek_with(period, |_| Some(reset_to)).or_re_throw()
    }

    /// The cell's value in `period`, without writing: what
    /// [`enter_with`](Self::enter_with) would leave it holding.
    pub fn peek_with(
        &self,
        period: Period,
        rule: impl FnOnce(Lapsed) -> Option<i128>,
    ) -> i128 {
        self.try_peek_with(period, rule).or_re_throw()
    }

    /// Like [`peek_with`](Self::peek_with), but report failure as `Err`
    /// instead of aborting.
    pub fn try_peek_with(
        &self,
        period: Period,
        rule: impl FnOnce(Lapsed) -> Option<i128>,
    ) -> Result<i128, LibraryExn> {
        let Loaded { tag, set, add } = L::load(&self.keys)?;
        if !tag.is_some_and(|tag| L::same(tag, period)) {
            let lapsed = Lapsed {
                value: set,
                from: tag,
                to: period.0,
            };
            if let Some(value) = rule(lapsed) {
                return Ok(value);
            }
        }
        sum(set, add)
    }

    /// What is stored, whatever the period: the tag (`None` if never
    /// written) and the value. For tests and diagnostics.
    pub fn recorded(&self) -> (Option<u64>, i128) {
        let Loaded { tag, set, add } = L::load(&self.keys).or_re_throw();
        (tag, sum(set, add).or_re_throw())
    }

    /// Set the cell to `value` for `period`, whatever it held.
    ///
    /// This is not a rollover: transactions in the same block that enter
    /// the cell compute a different reset and conflict with it. Use it to
    /// lay the cell out, or when the terms of its rule change.
    ///
    /// *Throws:* [`LibraryExn::RolloverAlreadyWritten`] if this transaction
    /// has added to the cell; a `value` outside `0..=MAX` as an underflow
    /// or overflow.
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
        let (_, add) = nnint::try_get_values(L::cell(&self.keys).get())?;
        if add != 0 {
            return Err(LibraryExn::RolloverAlreadyWritten);
        }
        L::reset(&mut self.keys, period, in_range::<L>(value)?)
    }
}

/// A [`Rollover`] cell that has been brought into the current period: the
/// only way to add to one.
pub struct Current<'a, L: TagLayout = Split> {
    cell: &'a mut Rollover<L>,
}

impl<L: TagLayout> Current<'_, L> {
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
        let Loaded { set, add, .. } = L::load(&self.cell.keys)?;
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
        let key = L::cell(&self.cell.keys);
        let (set, add) = nnint::try_get_values(key.get())?;
        let add = add.checked_add(L::scale(delta)?).ok_or(if delta < 0 {
            LibraryExn::NonNegativeIntUnderflow
        } else {
            LibraryExn::NonNegativeIntOverflow
        })?;
        nnint::try_set_add(key.get(), set, add, key.is_exclusive())
    }
}

impl<L: TagLayout> super::sealed::Sealed for Rollover<L> {}
impl<L: TagLayout> super::node::StorageType for Rollover<L> {}

impl<L: TagLayout> super::deletable::Deletable for Rollover<L> {
    fn delete_internal(&mut self, exclusive: bool) {
        L::delete(&mut self.keys, exclusive);
    }
}
