use groundhog_xdr::storage_item::StorageDeltaFlags;

/// Trait for storage types whose data can be fully deleted.
///
/// Leaf types (NNInt, Value, Bytestring) delete their single storage key.
/// Compound types implement this recursively when their elements are
/// Deletable. Types like Map that cannot enumerate entries do not
/// implement this trait.
pub trait Deletable {
    /// Delete storage for this node.
    ///
    /// Implementors OR the incoming `flags` with their keypath's own
    /// exclusive flag before issuing the syscall. This makes exclusivity
    /// additive: calling `delete()` on a slot accessed via
    /// `at_mut_exclusive()` still uses exclusive locking.
    fn delete_internal(&mut self, flags: StorageDeltaFlags);

    /// Delete using the keypath's own locking semantics.
    fn delete(&mut self) {
        self.delete_internal(StorageDeltaFlags::None);
    }

    /// Delete with an exclusive lock, regardless of keypath flag.
    fn delete_exclusive(&mut self) {
        self.delete_internal(StorageDeltaFlags::ExclusiveLock);
    }
}
