use super::keypath::{Keypath, LeafKey};
use crate::exn::nnint_overflow;
use crate::sys_exn::nnint::*;
use crate::sys_exn::nnint_internal as nnint;
use core::fmt::Debug;
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// A stored integer with a **commit-time floor**, not a call-time
/// non-negativity invariant.
///
/// The value is `set + add`: `set` is the last explicit [`set`](Self::set) (or
/// the value carried from the previous transaction) and `add` is what
/// [`inc`](Self::inc) has accumulated this transaction. The storage layer
/// enforces, at commit, that `set + add >= min(set, 0)`:
///
/// - With a non-negative (or never-)`set`, the floor is `0`: the value can't be
///   driven below zero, and a transaction that would commit a negative value
///   reverts.
/// - You may [`set`](Self::set) a *negative* value; the floor becomes that
///   value. The counter can hold it, but increments still can't take it any
///   lower than the value you set.
///
/// Crucially, for [`inc`](Self::inc) this is a *commit-time* constraint, not a
/// call-time error. For a hard, call-time "never negative" check with a clear
/// error (e.g. an ERC20 debit), use [`strict_inc`](Self::strict_inc) /
/// [`try_strict_inc`](Self::try_strict_inc). The exact rules (and concurrency
/// constraints) are on the `non_negative_int_*` free functions in
/// `sys_exn::nnint`.
#[derive(Debug)]
pub struct NNInt {
    key: LeafKey,
}

impl super::node::StorageHandle for NNInt {
    fn from_keypath(keypath: Keypath) -> Self {
        Self {
            key: LeafKey::from(keypath),
        }
    }
}

impl NNInt {
    /// Get the current value.  Aborts if location not yet set.
    pub fn get(&self) -> i128 {
        non_negative_int_get(self.key.get())
    }

    /// Get the current value, reporting failure as `Err` instead of
    /// aborting (the `try_` law; see the [`exn`](mod@crate::exn) module
    /// docs). A not-yet-set location is an `Err` for which
    /// [`is_storage_missing`](crate::exn::LibraryExn::is_storage_missing)
    /// is true — use
    /// [`none_if_missing`](crate::exn::NoneIfMissing::none_if_missing) to
    /// treat just that case as an ordinary `None`. See
    /// [`non_negative_int_try_get`] for the full error contract.
    pub fn try_get(&self) -> Result<i128, crate::exn::LibraryExn> {
        non_negative_int_try_get(self.key.get())
    }

    fn flags(&self) -> StorageDeltaFlags {
        self.key.flags()
    }

    /// Set the value, clearing accumulated [`inc`](Self::inc)s this transaction.
    ///
    /// A negative `set_value` is allowed: the commit-time floor becomes that
    /// value, so the counter can hold it but increments still can't take it any
    /// lower. A non-negative `set_value` keeps the floor at `0`. (At commit,
    /// `value >= min(set_value, 0)` must hold.) See the [type docs](NNInt).
    pub fn set(&mut self, set_value: i128) {
        nnint::set_add(self.key.get(), Some(set_value), 0, self.flags());
    }

    /// Increment by an amount (without querying the current value).
    ///
    /// The floor is enforced at *commit*, not here: the transaction reverts if
    /// the committed value would fall below `min(set, 0)` (see the
    /// [type docs](NNInt)). For a call-time non-negativity check, use
    /// [`strict_inc`](Self::strict_inc). Throws on overflow.
    pub fn inc(&mut self, increment: i128) {
        let key = self.key.get();
        let (s, a) = nnint::get_values(key);
        let add_value =
            a.checked_add(increment).unwrap_or_else(|| nnint_overflow());
        nnint::set_add(key, s, add_value, self.flags());
    }

    /// Increment by an amount, throwing in case of a negative result.
    pub fn strict_inc(&mut self, increment: i128) {
        use crate::re_throw::OrReThrow;
        self.try_strict_inc(increment).or_re_throw()
    }

    /// Increment by an amount, like [`NNInt::strict_inc`], but report
    /// failure to the caller instead of aborting (the `try_` law; see the
    /// [`exn`](mod@crate::exn) module docs), so a contract can raise a
    /// domain-specific exception. On `Err` the stored value is unmodified;
    /// no extra storage roundtrip over [`NNInt::inc`]. The domain errors
    /// are `NonNegativeIntOverflow`/`NonNegativeIntUnderflow`; a wrong
    /// storage type at the key or any other syscall failure surfaces as
    /// `LibraryExn::Syscall`.
    pub fn try_strict_inc(
        &mut self,
        increment: i128,
    ) -> Result<(), crate::exn::LibraryExn> {
        use crate::exn::LibraryExn;

        let key = self.key.get();
        let (s, a) = match nnint::try_get_values_force(key) {
            Ok(values) => values,
            // A never-written location increments as if from zero.
            Err(e) if e.is_storage_missing() => (0, 0),
            Err(e) => return Err(e),
        };

        let add_value = a
            .checked_add(increment)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;
        let total = s
            .checked_add(add_value)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;

        if total < 0 {
            return Err(LibraryExn::NonNegativeIntUnderflow);
        }

        nnint::try_set_add(key, Some(s), add_value, self.flags())
    }
}

impl super::sealed::Sealed for NNInt {}
impl super::node::StorageType for NNInt {}

impl super::deletable::Deletable for NNInt {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let effective = StorageDeltaFlags(flags.0 | self.flags().0);
        crate::sys_exn::delete::delete_with_flags(self.key.get(), effective);
    }
}
