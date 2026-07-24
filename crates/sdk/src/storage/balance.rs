use super::keypath::{KeyCache, Keypath};
use crate::exn::LibraryExn;
use crate::re_throw::OrReThrow;
use crate::sys_exn::nnint_internal as nnint;
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// A non-negative, zero-defaulting balance backed by Groundhog's mergeable
/// integer-storage cell.
///
/// Integer values in the range `0..i128::MAX` can be represented.
///
/// Increases and decreases have deliberately different APIs:
///
/// - [`increase`](Self::increase) records a positive delta without loading the
///   committed balance. Increases from concurrent transactions can merge;
///   the runtime checks final overflow when the transaction commits.
/// - [`decrease`](Self::decrease) always loads the committed balance and fails
///   immediately if the decrease would make it negative.
///
/// The alternative operation [`checked_increase`](Self::checked_increase)
/// loads the committed balance and fails immediately (during contract
/// execution) in the case of overflow.  This can make it easier to locate the
/// cause of failures.
///
/// Unlike [`NNInt`](super::NNInt), `Balance` exposes no absolute `set` or
/// signed adjustment operation. A missing storage cell reads as zero.
///
/// `Balance` uses the same storage representation as `NNInt`, so an existing
/// non-negative `NNInt` can be read as a balance. Setting that cell negative
/// through the lower-level API violates the `Balance` invariant. In
/// particular, `increase` does not load the committed value and therefore
/// cannot detect invalid negative state introduced through another handle
/// type.
#[derive(Debug)]
pub struct Balance {
    key: KeyCache,
}

impl From<Keypath> for Balance {
    fn from(keypath: Keypath) -> Self {
        Self {
            key: KeyCache::from(keypath),
        }
    }
}

impl Balance {
    pub const MAX: u128 = i128::MAX as u128;

    fn flags(&self) -> StorageDeltaFlags {
        self.key.keypath().flags()
    }

    /// Get the balance, treating a never-written location as zero.
    pub fn get(&self) -> u128 {
        self.try_get().or_re_throw()
    }

    /// Get the balance, reporting storage failures instead of throwing an
    /// exception. A never-written location is `Ok(0)`. A negative value
    /// written through the lower-level NNInt API is reported as an underflow.
    pub fn try_get(&self) -> Result<u128, LibraryExn> {
        match nnint::try_get_values_force(self.key.get()) {
            Ok((set, add)) => {
                let value = set
                    .checked_add(add)
                    .ok_or(LibraryExn::NonNegativeIntOverflow)?;
                value
                    .try_into()
                    .map_err(|_| LibraryExn::NonNegativeIntUnderflow)
            }
            Err(e) if e.is_storage_missing() => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// Increase by a non-negative amount without loading the current balance.
    ///
    /// This operation is deliberately mergeable: overflow against the
    /// committed balance is checked when the transaction commits, not here.
    /// Use [`checked_increase`](Self::checked_increase) when the contract
    /// needs an immediate overflow result.
    ///
    /// *Throws:* any error that would be returned by
    /// [`try_increase`](Self::try_increase).
    pub fn increase(&mut self, amount: u128) {
        self.try_increase(amount).or_re_throw()
    }

    /// Like [`increase`](Self::increase), but report immediate conversion or
    /// accumulated-delta overflow as `Err` instead of aborting (the `try_`
    /// law; see the [`exn`](mod@crate::exn) module docs).
    ///
    /// This does not turn the increase into checked arithmetic. Overflow
    /// against the committed balance remains a commit-time check because an
    /// increase deliberately does not load that balance.
    pub fn try_increase(&mut self, amount: u128) -> Result<(), LibraryExn> {
        if amount == 0 {
            return Ok(());
        }
        let increment: i128 = amount
            .try_into()
            .map_err(|_| LibraryExn::NonNegativeIntOverflow)?;
        let key = self.key.get();
        let (set, add) = nnint::try_get_values(key)?;
        let add = add
            .checked_add(increment)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;
        nnint::try_set_add(key, set, add, self.flags())
    }

    /// Increase by a non-negative amount, throwing immediately if the resulting
    /// balance would overflow.
    ///
    /// Unlike [`increase`](Self::increase), this loads the current balance and
    /// therefore establishes a strict constraint against the state snapshot.
    ///
    /// *Throws:* any error that would be returned by
    /// [`try_checked_increase`](Self::try_checked_increase).
    pub fn checked_increase(&mut self, amount: u128) {
        self.try_checked_increase(amount).or_re_throw()
    }

    /// Like [`checked_increase`](Self::checked_increase), but report overflow
    /// or storage-type failure as `Err` instead of aborting (the `try_` law; see
    /// the [`exn`](mod@crate::exn) module docs). On `Err`, the stored value is
    /// unmodified.
    pub fn try_checked_increase(
        &mut self,
        amount: u128,
    ) -> Result<(), LibraryExn> {
        if amount == 0 {
            return Ok(());
        }
        let increment: i128 = amount
            .try_into()
            .map_err(|_| LibraryExn::NonNegativeIntOverflow)?;
        let key = self.key.get();
        let (set, add) = match nnint::try_get_values_force(key) {
            Ok(values) => values,
            Err(e) if e.is_storage_missing() => (0, 0),
            Err(e) => return Err(e),
        };
        let add = add
            .checked_add(increment)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;
        let total = set
            .checked_add(add)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;
        if total < 0 {
            return Err(LibraryExn::NonNegativeIntUnderflow);
        }
        nnint::try_set_add(key, Some(set), add, self.flags())
    }

    /// Decrease by a non-negative amount, failing immediately if the balance
    /// is insufficient.
    ///
    /// *Throws:* any error that would be returned by `try_decrease`.
    pub fn decrease(&mut self, amount: u128) {
        self.try_decrease(amount).or_re_throw()
    }

    /// Like [`decrease`](Self::decrease), but report underflow or storage
    /// failure as `Err` instead of aborting (the `try_` law; see the
    /// [`exn`](mod@crate::exn) module docs).
    ///
    /// On `Err`, the stored value is unmodified.
    pub fn try_decrease(&mut self, amount: u128) -> Result<(), LibraryExn> {
        if amount == 0 {
            return Ok(());
        }
        let decrement: i128 = amount
            .try_into()
            .map_err(|_| LibraryExn::NonNegativeIntUnderflow)?;
        let key = self.key.get();
        let (set, add) = match nnint::try_get_values_force(key) {
            Ok(values) => values,
            Err(e) if e.is_storage_missing() => (0, 0),
            Err(e) => return Err(e),
        };
        let add = add
            .checked_sub(decrement)
            .ok_or(LibraryExn::NonNegativeIntUnderflow)?;
        let total = set
            .checked_add(add)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;
        if total < 0 {
            return Err(LibraryExn::NonNegativeIntUnderflow);
        }
        nnint::try_set_add(key, Some(set), add, self.flags())
    }
}

impl super::sealed::Sealed for Balance {}
impl super::node::StorageType for Balance {}

impl super::deletable::Deletable for Balance {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let effective = StorageDeltaFlags(flags.0 | self.flags().0);
        crate::sys_exn::delete::delete_with_flags(self.key.get(), effective);
    }
}
