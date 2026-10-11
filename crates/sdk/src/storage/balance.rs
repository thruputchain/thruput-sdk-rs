use super::keypath::{Keypath, LeafKey};
use crate::exn::LibraryExn;
use crate::re_throw::OrReThrow;
use crate::sys_exn::nnint_internal as nnint;

/// A non-negative, zero-defaulting balance backed by Thruput's mergeable
/// integer-storage cell.
///
/// Values are in `0..=i128::MAX`. Operations reject a negative amount with
/// [`LibraryExn::NonNegativeIntUnderflow`].
///
/// Each direction comes in a checked and a lazy form:
///
/// - [`increase`](Self::increase) and [`decrease`](Self::decrease) load the
///   committed balance and fail immediately (during contract execution) if the
///   result would overflow or go negative. Loading establishes a strict
///   constraint against the state snapshot, and immediate failure makes the
///   cause easier to locate.
/// - [`lazy_increase`](Self::lazy_increase) and
///   [`lazy_decrease`](Self::lazy_decrease) record a delta without loading the
///   committed balance. Deltas from concurrent transactions can merge; the
///   runtime checks the committed result when the transaction commits.
///
/// Unlike [`NNInt`](super::NNInt), `Balance` exposes no absolute `set` or
/// signed-adjustment operation. A missing storage cell reads as zero.
///
/// `Balance` uses the same storage representation as `NNInt`, so an existing
/// non-negative `NNInt` can be read as a balance. Setting that cell negative
/// through the lower-level API violates the `Balance` invariant. In
/// particular, the lazy operations do not load the committed value and
/// therefore cannot detect invalid negative state introduced through another
/// handle type.
#[derive(Debug)]
pub struct Balance {
    key: LeafKey,
}

impl From<Keypath> for Balance {
    fn from(keypath: Keypath) -> Self {
        Self {
            key: LeafKey::from(keypath),
        }
    }
}

impl Balance {
    pub const MAX: i128 = i128::MAX;

    /// Get the balance, treating a never-written location as zero.
    pub fn get(&self) -> i128 {
        self.try_get().or_re_throw()
    }

    /// Get the balance, reporting storage failures instead of throwing an
    /// exception. A never-written location is `Ok(0)`. A negative value
    /// written through the lower-level NNInt API is reported as an underflow.
    pub fn try_get(&self) -> Result<i128, LibraryExn> {
        match nnint::try_get_values_force(self.key.get()) {
            Ok((set, add)) => {
                let value = set
                    .checked_add(add)
                    .ok_or(LibraryExn::NonNegativeIntOverflow)?;
                if value < 0 {
                    return Err(LibraryExn::NonNegativeIntUnderflow);
                }
                Ok(value)
            }
            Err(e) if e.is_storage_missing() => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// Increase by a non-negative amount, throwing immediately if the resulting
    /// balance would overflow.
    ///
    /// This loads the current balance and therefore establishes a strict
    /// constraint against the state snapshot. Use
    /// [`lazy_increase`](Self::lazy_increase) when the increase should instead
    /// merge with concurrent transactions.
    ///
    /// *Throws:* any error that would be returned by
    /// [`try_increase`](Self::try_increase).
    pub fn increase(&mut self, amount: i128) {
        self.try_increase(amount).or_re_throw()
    }

    /// Like [`increase`](Self::increase), but report overflow or storage-type
    /// failure as `Err` instead of aborting (the `try_` law; see the
    /// [`exn`](mod@crate::exn) module docs). On `Err`, the stored value is
    /// unmodified.
    pub fn try_increase(&mut self, amount: i128) -> Result<(), LibraryExn> {
        if amount == 0 {
            return Ok(());
        }
        if amount < 0 {
            return Err(LibraryExn::NonNegativeIntUnderflow);
        }
        let key = self.key.get();
        let (set, add) = match nnint::try_get_values_force(key) {
            Ok(values) => values,
            Err(e) if e.is_storage_missing() => (0, 0),
            Err(e) => return Err(e),
        };
        let add = add
            .checked_add(amount)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;
        let total = set
            .checked_add(add)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;
        if total < 0 {
            return Err(LibraryExn::NonNegativeIntUnderflow);
        }
        nnint::try_set_add(key, Some(set), add, self.key.is_exclusive())
    }

    /// Decrease by a non-negative amount, throwing immediately if the balance
    /// is insufficient.
    ///
    /// This loads the current balance and therefore establishes a strict
    /// constraint against the state snapshot. Use
    /// [`lazy_decrease`](Self::lazy_decrease) when the decrease should instead
    /// merge with concurrent transactions.
    ///
    /// *Throws:* any error that would be returned by
    /// [`try_decrease`](Self::try_decrease).
    pub fn decrease(&mut self, amount: i128) {
        self.try_decrease(amount).or_re_throw()
    }

    /// Like [`decrease`](Self::decrease), but report underflow or storage
    /// failure as `Err` instead of aborting (the `try_` law; see the
    /// [`exn`](mod@crate::exn) module docs).
    ///
    /// On `Err`, the stored value is unmodified.
    pub fn try_decrease(&mut self, amount: i128) -> Result<(), LibraryExn> {
        if amount == 0 {
            return Ok(());
        }
        if amount < 0 {
            return Err(LibraryExn::NonNegativeIntUnderflow);
        }
        let key = self.key.get();
        let (set, add) = match nnint::try_get_values_force(key) {
            Ok(values) => values,
            Err(e) if e.is_storage_missing() => (0, 0),
            Err(e) => return Err(e),
        };
        let add = add
            .checked_sub(amount)
            .ok_or(LibraryExn::NonNegativeIntUnderflow)?;
        let total = set
            .checked_add(add)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;
        if total < 0 {
            return Err(LibraryExn::NonNegativeIntUnderflow);
        }
        nnint::try_set_add(key, Some(set), add, self.key.is_exclusive())
    }

    /// Increase by a non-negative amount without loading the current balance.
    ///
    /// This operation is deliberately mergeable: overflow against the
    /// committed balance is checked when the transaction commits, not here.
    /// Use [`increase`](Self::increase) when the contract needs an immediate
    /// overflow result.
    ///
    /// *Throws:* any error that would be returned by
    /// [`try_lazy_increase`](Self::try_lazy_increase).
    pub fn lazy_increase(&mut self, amount: i128) {
        self.try_lazy_increase(amount).or_re_throw()
    }

    /// Like [`lazy_increase`](Self::lazy_increase), but report immediate
    /// conversion or accumulated-delta overflow as `Err` instead of aborting
    /// (the `try_` law; see the [`exn`](mod@crate::exn) module docs).
    ///
    /// This does not turn the increase into checked arithmetic. Overflow
    /// against the committed balance remains a commit-time check because a
    /// lazy increase deliberately does not load that balance.
    pub fn try_lazy_increase(
        &mut self,
        amount: i128,
    ) -> Result<(), LibraryExn> {
        if amount == 0 {
            return Ok(());
        }
        if amount < 0 {
            return Err(LibraryExn::NonNegativeIntUnderflow);
        }
        let key = self.key.get();
        let (set, add) = nnint::try_get_values(key)?;
        let add = add
            .checked_add(amount)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;
        nnint::try_set_add(key, set, add, self.key.is_exclusive())
    }

    /// Decrease by a non-negative amount without loading the current balance.
    ///
    /// This operation is deliberately mergeable: sufficiency against the
    /// committed balance is checked when the transaction commits, not here.
    /// Use [`decrease`](Self::decrease) when the contract needs an immediate
    /// insufficient-balance result.
    ///
    /// *Throws:* any error that would be returned by
    /// [`try_lazy_decrease`](Self::try_lazy_decrease).
    pub fn lazy_decrease(&mut self, amount: i128) {
        self.try_lazy_decrease(amount).or_re_throw()
    }

    /// Like [`lazy_decrease`](Self::lazy_decrease), but report immediate
    /// conversion or accumulated-delta underflow as `Err` instead of aborting
    /// (the `try_` law; see the [`exn`](mod@crate::exn) module docs).
    ///
    /// This does not turn the decrease into checked arithmetic. Sufficiency
    /// against the committed balance remains a commit-time check because a
    /// lazy decrease deliberately does not load that balance. A balance
    /// already written this transaction is the exception: that value is
    /// pending in the storage layer rather than in the database, so it costs
    /// no load and is checked here.
    pub fn try_lazy_decrease(
        &mut self,
        amount: i128,
    ) -> Result<(), LibraryExn> {
        if amount == 0 {
            return Ok(());
        }
        if amount < 0 {
            return Err(LibraryExn::NonNegativeIntUnderflow);
        }
        let key = self.key.get();
        let (set, add) = nnint::try_get_values(key)?;
        let add = add
            .checked_sub(amount)
            .ok_or(LibraryExn::NonNegativeIntUnderflow)?;
        if let Some(set) = set {
            let total = set
                .checked_add(add)
                .ok_or(LibraryExn::NonNegativeIntOverflow)?;
            if total < 0 {
                return Err(LibraryExn::NonNegativeIntUnderflow);
            }
        }
        nnint::try_set_add(key, set, add, self.key.is_exclusive())
    }
}

impl super::sealed::Sealed for Balance {}
impl super::node::StorageType for Balance {}

impl super::flags::Flaggable for Balance {
    fn storage_key(&self) -> &crate::types::StorageKey {
        self.key.get()
    }
}

impl super::deletable::Deletable for Balance {
    fn delete_internal(&mut self, exclusive: bool) {
        let exclusive = exclusive || self.key.is_exclusive();
        crate::sys_exn::delete::delete(self.key.get(), exclusive);
    }
}
