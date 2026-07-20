use super::keypath::{KeyCache, Keypath};
use crate::exn::LibraryExn;
use crate::re_throw::OrReThrow;
use crate::sys_exn::nnint_internal as nnint;
use crate::u127_type::*;
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// A non-negative, zero-defaulting balance backed by Groundhog's mergeable
/// integer storage cell.
///
/// Credits and debits have deliberately different APIs:
///
/// - [`credit`](Self::credit) records a positive delta without loading the
///   current balance. Credits from multiple transactions can therefore merge;
///   the runtime checks final overflow when the transaction commits.
/// - [`debit`](Self::debit) loads the current balance and fails immediately if
///   the debit would make it negative.
/// - [`checked_credit`](Self::checked_credit) is the explicit alternative when
///   a caller needs to detect overflow during contract execution.
///
/// Unlike [`NNInt`](super::NNInt), `Balance` exposes no absolute `set` or
/// signed adjustment operation. A missing storage cell reads as zero.
///
/// `Balance` uses the same storage representation as `NNInt`, so an existing
/// non-negative `NNInt` can be read as a balance. Setting that cell negative
/// through the lower-level API violates the `Balance` invariant. In particular,
/// a lazy credit does not load the committed value and therefore cannot detect
/// invalid negative state introduced through another handle type.
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
    fn flags(&self) -> StorageDeltaFlags {
        self.key.keypath().flags()
    }

    /// Get the balance, treating a never-written location as zero.
    pub fn get(&self) -> u128 {
        self.try_get().or_re_throw()
    }

    /// Get the balance, reporting storage failures instead of throwing.
    /// A never-written location is `Ok(0)`. A negative value written through
    /// the lower-level NNInt API is reported as an underflow.
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

    /// Credit a non-negative amount without loading the current balance.
    ///
    /// This operation is deliberately mergeable: overflow against the
    /// committed balance is checked when the transaction commits, not here.
    /// Use [`checked_credit`](Self::checked_credit) when the contract needs an
    /// immediate overflow result.
    pub fn credit(&mut self, amount: u127) {
        self.try_credit(amount).or_re_throw()
    }

    /// Like [`credit`](Self::credit), but report immediate conversion or
    /// accumulated-delta overflow as `Err` instead of aborting (the `try_`
    /// law; see the [`exn`](mod@crate::exn) module docs).
    ///
    /// This does not turn the credit into checked arithmetic. Overflow against
    /// the committed balance remains a commit-time check because a credit
    /// deliberately does not load that balance.
    pub fn try_credit(&mut self, amount: u127) -> Result<(), LibraryExn> {
        if amount.is_zero() {
            return Ok(());
        }
        let key = self.key.get();
        let (set, add) = nnint::try_get_values(key)?;
        let add = add
            .checked_add(amount.into())
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;
        nnint::try_set_add(key, set, add, self.flags())
    }

    /// Credit a non-negative amount, throwing immediately if the resulting
    /// balance would overflow.
    ///
    /// Unlike [`credit`](Self::credit), this loads the current balance and
    /// therefore establishes a strict constraint against the state snapshot.
    pub fn checked_credit(&mut self, amount: u127) {
        self.try_checked_credit(amount).or_re_throw()
    }

    /// Like [`checked_credit`](Self::checked_credit), but report overflow or
    /// storage failure as `Err` instead of aborting (the `try_` law; see the
    /// [`exn`](mod@crate::exn) module docs). On `Err`, the stored value is
    /// unmodified.
    pub fn try_checked_credit(
        &mut self,
        amount: u127,
    ) -> Result<(), LibraryExn> {
        if amount.is_zero() {
            return Ok(());
        }
        let key = self.key.get();
        let (set, add) = match nnint::try_get_values_force(key) {
            Ok(values) => values,
            Err(e) if e.is_storage_missing() => (0, 0),
            Err(e) => return Err(e),
        };
        let amount: i128 = amount.into();
        let add = add
            .checked_add(amount)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;
        let total = set
            .checked_add(add)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;
        if total < 0 {
            return Err(LibraryExn::NonNegativeIntUnderflow);
        }
        nnint::try_set_add(key, Some(set), add, self.flags())
    }

    /// Debit a non-negative amount, throwing immediately if the balance is
    /// insufficient.
    pub fn debit(&mut self, amount: u127) {
        self.try_debit(amount).or_re_throw()
    }

    /// Like [`debit`](Self::debit), but report underflow or storage failure as
    /// `Err` instead of aborting (the `try_` law; see the
    /// [`exn`](mod@crate::exn) module docs). On `Err`, the stored value is
    /// unmodified.
    pub fn try_debit(&mut self, amount: u127) -> Result<(), LibraryExn> {
        if amount.is_zero() {
            return Ok(());
        }
        let key = self.key.get();
        let (set, add) = match nnint::try_get_values_force(key) {
            Ok(values) => values,
            Err(e) if e.is_storage_missing() => (0, 0),
            Err(e) => return Err(e),
        };
        let amount: i128 = amount.into();
        let add = add
            .checked_sub(amount)
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
