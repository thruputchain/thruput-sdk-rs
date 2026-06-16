use super::keypath::Keypath;
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
    keypath: Keypath,
}

impl From<Keypath> for NNInt {
    fn from(keypath: Keypath) -> Self {
        Self { keypath }
    }
}

impl NNInt {
    /// Get the current value.  Aborts if location not yet set.
    pub fn get(&self) -> i128 {
        non_negative_int_get(&self.keypath.generate_key())
    }

    /// Get the current value, or none if not yet set
    pub fn try_get(&self) -> Option<i128> {
        non_negative_int_try_get(&self.keypath.generate_key())
    }

    fn flags(&self) -> StorageDeltaFlags {
        if self.keypath.is_exclusive() {
            StorageDeltaFlags::ExclusiveLock
        } else {
            StorageDeltaFlags::None
        }
    }

    /// Set the value, clearing accumulated [`inc`](Self::inc)s this transaction.
    ///
    /// A negative `set_value` is allowed: the commit-time floor becomes that
    /// value, so the counter can hold it but increments still can't take it any
    /// lower. A non-negative `set_value` keeps the floor at `0`. (At commit,
    /// `value >= min(set_value, 0)` must hold.) See the [type docs](NNInt).
    pub fn set(&mut self, set_value: i128) {
        nnint::set_add(
            &self.keypath.generate_key(),
            Some(set_value),
            0,
            self.flags(),
        );
    }

    /// Increment by an amount (without querying the current value).
    ///
    /// The floor is enforced at *commit*, not here: the transaction reverts if
    /// the committed value would fall below `min(set, 0)` (see the
    /// [type docs](NNInt)). For a call-time non-negativity check, use
    /// [`strict_inc`](Self::strict_inc). Throws on overflow.
    pub fn inc(&mut self, increment: i128) {
        let key = &self.keypath.generate_key();
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
    /// overflow/underflow to the caller instead of aborting, so a contract
    /// can raise a domain-specific exception. On `Err` the stored value is
    /// unmodified; no extra storage roundtrip over [`NNInt::inc`].
    pub fn try_strict_inc(
        &mut self,
        increment: i128,
    ) -> Result<(), crate::exn::LibraryExn> {
        use crate::exn::LibraryExn;

        let key = &self.keypath.generate_key();
        let (s, a) = nnint::try_get_values_force(key).unwrap_or((0, 0));

        let add_value = a
            .checked_add(increment)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;
        let total = s
            .checked_add(add_value)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;

        if total < 0 {
            return Err(LibraryExn::NonNegativeIntUnderflow);
        }

        nnint::set_add(key, Some(s), add_value, self.flags());
        Ok(())
    }
}

impl super::sealed::Sealed for NNInt {}
impl super::node::StorageType for NNInt {}

impl super::deletable::Deletable for NNInt {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let effective = StorageDeltaFlags(flags.0 | self.flags().0);
        crate::sys_exn::delete::delete_with_flags(
            &self.keypath.generate_key(),
            effective,
        );
    }
}
