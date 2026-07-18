use super::keypath::{KeyCache, Keypath};
use crate::exn::nnint_overflow;
use crate::sys_exn::nnint::*;
use crate::sys_exn::nnint_internal as nnint;
use core::fmt::Debug;
use groundhog_xdr::storage_item::StorageDeltaFlags;


/// Implements a linear constraint on some set of (contract-defined) state.
/// Specifically, this implements constraints of the form \Sum_i a_i x_i <= b.
///
/// The constraint is guaranteed to hold at the state snapshot taken at the end of each block.
/// Contracts express not their state variables directly, but the changes in state variables
/// that a transaction intends to make.
///
/// Some kinds of constraints can be expressed directly as checks within one transaction.
/// This data type is not for those kinds of constraints.  Rather, this type is how a contract
/// can tell the Groundhog runtime about constraints that must hold across transactions in a block.
/// For example, an aggregate velocity limit on a user's token balance cannot be checked within a single transaction,
/// but can be enforced by the Groundhog runtime when packing new blocks of transactions.
///
/// The arithmetic is performed by the caller; so the arguments are the
/// sum (for a given transaction) of a_i * \delta_i.  
///
/// Callers can reset the constraint via `set_capacity`.
#[derive(Debug)]
pub struct LinearConstraint {
    key: KeyCache,
    delta: i128,
    capacity: Option<i128>,
    _live: crate::storage::live_guards::LiveGuard,
}

impl From<Keypath> for LinearConstraint {
    fn from(keypath: Keypath) -> Self {
        Self {
            key: KeyCache::from(keypath),
            _live: crate::storage::live_guards::LiveGuard::new(),
        }
    }
}

impl LinearConstraint {
    /// Get the remaining capacity.
    ///
    /// If the storage location has not yet been loaded, the remaining capacity is assumed to be 0.
    ///
    /// Warning: This value might return a negative value, if the constraint has been violated within
    /// current transaction.  A transaction cannot successfully commit in such a state.
    /// However, to allow for flexibility in contract implementations,
    /// (i.e. looping over `LinearConstraint::apply_delta` in a transaction),
    /// we allow a contract to optionally temporarily violate its constraint,
    /// on the knowledge that either the transaction will fail or the the contract will repair the constraint
    /// before terminating.
    pub fn try_get_remaining_capacity(&self) -> Result<i128, crate::exn::LibraryExn> {
        let (s, a) = match nnint::try_get_values(self.key.get()) {
            Ok((s, a)) => (s, a),
            Err(e) if e.is_storage_missing() => (0, 0),
            Err(e) => return Err(e),
        };
        let remaining = s.checked_add(a).ok_or(crate::exn::LibraryExn::NonNegativeIntOverflow)?;
        Ok(remaining)
    }

    pub fn get_remaining_capacity(&self) -> i128 {
        self.try_get_remaining_capacity().or_re_throw()
    }

    pub fn try_apply_delta(&mut self, delta: i128) -> Result<(), crate::exn::LibraryExn> {
        self.delta = self.delta.checked_add(delta).ok_or(crate::exn::LibraryExn::NonNegativeIntOverflow)?;
        Ok(())
    }

    pub fn apply_delta(&mut self, delta: i128) {
        self.try_apply_delta(delta).or_re_throw();
    }
}

/// Lazy writeback to storage
impl Drop for LinearConstraint {
    fn drop(&mut self) {
        nnint::try_set(self.key.get(), self.delta, self.capacity).or_re_throw();
        // live guard drops here
    }
}

impl super::sealed::Sealed for LinearConstraint {}
impl super::node::StorageType for LinearConstraint {}

impl super::deletable::Deletable for LinearConstraint {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let effective = StorageDeltaFlags(flags.0 | self.flags().0);
        crate::sys_exn::delete::delete_with_flags(self.key.get(), effective);
    }
}
