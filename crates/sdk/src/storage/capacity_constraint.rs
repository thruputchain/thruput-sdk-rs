use super::keypath::{Keypath, LeafKey};
use crate::exn::LibraryExn;
use crate::re_throw::OrReThrow;
use crate::sys_exn::nnint_internal as nnint;

/// Implements a linear constraint on some set of (contract-defined) state.
/// Specifically, this implements constraints of the form `\Sum_i a_i x_i <= b`.
///
/// The constraint is guaranteed to hold at the state snapshot taken at the
/// end of each block. Contracts express not their state variables directly,
/// but the changes in state variables that a transaction intends to make.
///
/// Some kinds of constraints can be expressed directly as checks within one
/// transaction. This data type is not for those kinds of constraints.
/// Rather, this type is how a contract can tell the Groundhog runtime about
/// constraints that must hold across transactions in a block. For example,
/// an aggregate velocity limit on a user's token balance cannot be checked
/// within a single transaction, but can be enforced by the Groundhog runtime
/// when packing new blocks of transactions.
///
/// The storage cell tracks the *remaining capacity* (`b - \Sum_i a_i x_i`).
/// The weighted arithmetic is performed by the caller: pass
/// [`consume_capacity`](Self::consume_capacity) the sum (for a given
/// transaction) of `a_i * \delta_i`.
///
/// Callers can reset the constraint via
/// [`set_capacity`](Self::set_capacity), which must be the first write to
/// the cell in its transaction (or an idempotent repeat of the value
/// already set this transaction).
///
/// # Writeback
///
/// Changes are staged on the handle and flushed to storage when it drops,
/// *merging* with whatever earlier handles in the same transaction recorded.
/// A handle that only read is not flushed at all: read-only access neither
/// writes storage nor counts as a prior write for
/// [`set_capacity`](Self::set_capacity). To catch a flush error instead of
/// aborting in drop, flush explicitly with [`try_flush`](Self::try_flush).
pub struct CapacityConstraint {
    key: LeafKey,
    capacity: Option<i128>,
    delta: i128,
    _live: crate::storage::live_guards::LiveGuard,
}

impl From<Keypath> for CapacityConstraint {
    fn from(keypath: Keypath) -> Self {
        Self {
            key: LeafKey::from(keypath),
            capacity: None,
            delta: 0,
            _live: crate::storage::live_guards::LiveGuard::new(),
        }
    }
}

impl CapacityConstraint {
    /// Get the remaining capacity: the capacity at the last
    /// [`set_capacity`](Self::set_capacity) (in this transaction, or
    /// committed by a previous one), plus all consumption recorded this
    /// transaction, including this handle's unflushed changes.
    ///
    /// If the storage location has never been written, the capacity
    /// baseline is 0.
    ///
    /// Warning: this might return a negative value, if the constraint has
    /// been violated within the current transaction. A transaction cannot
    /// successfully commit in such a state. However, to allow for
    /// flexibility in contract implementations (i.e. looping over
    /// [`consume_capacity`](Self::consume_capacity) in a transaction), we
    /// allow a contract to optionally temporarily violate its constraint,
    /// on the knowledge that either the transaction will fail or the
    /// contract will repair the constraint before terminating.
    pub fn try_get_remaining_capacity(
        &self,
    ) -> Result<i128, crate::exn::LibraryExn> {
        let (s, a) = match nnint::try_get_values_force(self.key.get()) {
            Ok(values) => values,
            Err(e) if e.is_storage_missing() => (0, 0),
            Err(e) => return Err(e),
        };
        self.capacity
            .unwrap_or(s)
            .checked_add(a)
            .and_then(|r| r.checked_add(self.delta))
            .ok_or(LibraryExn::NonNegativeIntOverflow)
    }

    /// Like
    /// [`try_get_remaining_capacity`](Self::try_get_remaining_capacity),
    /// but aborts on failure instead of reporting it (the `try_` law; see
    /// the [`exn`](mod@crate::exn) module docs).
    pub fn get_remaining_capacity(&self) -> i128 {
        self.try_get_remaining_capacity().or_re_throw()
    }

    /// Consume `amount` units of remaining capacity (without querying the
    /// current value). A negative `amount` releases capacity back.
    ///
    /// The constraint is enforced at *commit*, not here: consumption is
    /// staged on this handle, flushed to storage when it drops, and checked
    /// by the runtime when the block is packed. Throws only on arithmetic
    /// overflow of the staged consumption.
    pub fn consume_capacity(&mut self, amount: i128) {
        self.try_consume_capacity(amount).or_re_throw();
    }

    /// Like [`consume_capacity`](Self::consume_capacity), but report
    /// overflow as `Err` instead of aborting (the `try_` law; see the
    /// [`exn`](mod@crate::exn) module docs).
    pub fn try_consume_capacity(
        &mut self,
        amount: i128,
    ) -> Result<(), crate::exn::LibraryExn> {
        self.delta = self.delta.checked_sub(amount).ok_or(if amount > 0 {
            LibraryExn::NonNegativeIntUnderflow
        } else {
            LibraryExn::NonNegativeIntOverflow
        })?;
        Ok(())
    }

    /// Set the remaining capacity.
    ///
    /// This must be the first write to the constraint in its transaction,
    /// or an idempotent repeat of the capacity already set this
    /// transaction; see [`try_set_capacity`](Self::try_set_capacity).
    /// Aborts otherwise.
    pub fn set_capacity(&mut self, capacity: i128) {
        self.try_set_capacity(capacity).or_re_throw();
    }

    /// Like [`set_capacity`](Self::set_capacity), but report failure as
    /// `Err` instead of aborting (the `try_` law; see the
    /// [`exn`](mod@crate::exn) module docs).
    ///
    /// Setting the capacity must be the first write to the constraint in
    /// its transaction, with one exception: repeating the value already
    /// set this transaction (by this handle or one already dropped) is an
    /// idempotent no-op that preserves recorded consumption. This mirrors
    /// the runtime's cross-transaction rule — concurrent transactions
    /// setting the same value may share a block — so same-value sets
    /// commute at every scope. Any other prior write reports
    /// [`LibraryExn::CapacityConstraintAlreadyWritten`]: a different
    /// capacity value, or consumption recorded before any set. This
    /// prevents a capacity reset from silently discarding consumption.
    /// The check consults the pending storage state, not just this handle,
    /// so writes flushed by already-dropped handles are seen.
    pub fn try_set_capacity(
        &mut self,
        capacity: i128,
    ) -> Result<(), crate::exn::LibraryExn> {
        if let Some(staged) = self.capacity {
            return if staged == capacity {
                Ok(())
            } else {
                Err(LibraryExn::CapacityConstraintAlreadyWritten)
            };
        }
        let (s, a) = nnint::try_get_values(self.key.get())?;
        if let Some(pending) = s {
            return if pending == capacity {
                Ok(())
            } else {
                Err(LibraryExn::CapacityConstraintAlreadyWritten)
            };
        }
        // No set anywhere this transaction: consumption must not have been
        // recorded either, or the reset would discard it.
        if self.delta != 0 || a != 0 {
            return Err(LibraryExn::CapacityConstraintAlreadyWritten);
        }
        self.capacity = Some(capacity);
        Ok(())
    }

    /// Merge this handle's staged changes into the cell's pending values
    /// now instead of waiting for drop.
    ///
    /// A clean handle (nothing staged since construction or the last flush)
    /// is a no-op. A successful flush resets the staged delta to zero, so
    /// the handle remains usable: consumption staged by later
    /// [`consume_capacity`](Self::consume_capacity) calls is merged on top
    /// of the flushed batch by drop (or the next flush) — consume, flush,
    /// consume accumulates both consumptions.
    pub fn flush(&mut self) {
        self.try_flush().or_re_throw()
    }

    /// Like [`flush`](Self::flush), but report failure as `Err` instead of
    /// aborting (the `try_` law; see the [`exn`](mod@crate::exn) module
    /// docs) — in particular, the only way to catch an error the drop-time
    /// flush would abort with. On `Err` the staged changes are retained:
    /// drop re-attempts the flush and aborts if it still fails, so a caught
    /// error cannot silently lose the write.
    pub fn try_flush(&mut self) -> Result<(), crate::exn::LibraryExn> {
        if self.capacity.is_none() && self.delta == 0 {
            return Ok(());
        }
        let key = self.key.get();
        let (s, a) = nnint::try_get_values(key)?;
        let add_value = a
            .checked_add(self.delta)
            .ok_or(LibraryExn::NonNegativeIntOverflow)?;
        nnint::try_set_add(
            key,
            self.capacity.or(s),
            add_value,
            self.key.is_exclusive(),
        )?;
        self.capacity = None;
        self.delta = 0;
        Ok(())
    }
}

/// Lazy writeback to storage: staged changes are merged into the cell's
/// pending values when the handle drops. An untouched handle skips the
/// syscall entirely, so read-only access leaves no trace.
impl Drop for CapacityConstraint {
    fn drop(&mut self) {
        self.try_flush().or_re_throw();
        // live guard drops here
    }
}

impl super::sealed::Sealed for CapacityConstraint {}
impl super::node::StorageType for CapacityConstraint {}

impl super::deletable::Deletable for CapacityConstraint {
    fn delete_internal(&mut self, exclusive: bool) {
        // Discard staged changes: they must not be flushed on top of the
        // delete when this handle drops.
        self.capacity = None;
        self.delta = 0;
        let exclusive = exclusive || self.key.is_exclusive();
        crate::sys_exn::delete::delete(self.key.get(), exclusive);
    }
}
