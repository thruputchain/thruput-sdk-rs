/// Trait for storage types whose data can be fully deleted.
///
/// Leaf types (NNInt, Value, Bytestring) delete their single storage key.
/// Compound types implement this recursively when their elements are
/// Deletable. Types like Map that cannot enumerate entries do not
/// implement this trait.
pub trait Deletable {
    /// Delete storage for this node.
    ///
    /// `exclusive` is the deletion's write policy. Leaf implementors combine
    /// it with their keypath's own exclusivity and, if either is set, request
    /// an exclusive lock once the deletion succeeds; compound implementors
    /// pass it to each child. This makes exclusivity additive: calling
    /// `delete()` on a slot accessed via `at_mut_exclusive()` still locks the
    /// deletion.
    fn delete_internal(&mut self, exclusive: bool);

    /// Delete using the keypath's own locking semantics.
    fn delete(&mut self) {
        self.delete_internal(false);
    }

    /// Delete, then request an exclusive lock, regardless of keypath flag.
    /// Later writes in the transaction keep the lock.
    fn delete_exclusive(&mut self) {
        self.delete_internal(true);
    }
}
