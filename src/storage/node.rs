use super::keypath::Keypath;

/// Unifying trait for all storage-backed types.
///
/// Storage nodes are lightweight handles constructed from a keypath.
/// They may be leaf types (NNInt, Bytestring, Value<T>) whose methods
/// make immediate syscalls, or collection types (Map, Vector) that
/// produce child nodes via `at()` / `at_mut()`.
pub trait StorageNode: Sized {
    fn from_keypath(keypath: Keypath) -> Self;
    fn keypath(&self) -> &Keypath;
}
