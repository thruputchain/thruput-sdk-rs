use super::keypath::Keypath;
use super::sealed::Sealed;
use core::ops::{Deref, DerefMut};
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// Marker trait for types that can appear as elements in Map and Vector.
/// Sealed — only SDK storage types implement this.
#[allow(private_bounds)]
pub trait StorageType: Sealed + From<Keypath> {}

/// A keypath-storing wrapper for compound storage field groups.
///
/// Pairs a `T: From<Keypath>` with its root keypath.
/// Derefs to T for field access.
pub struct Node<T> {
    keypath: Keypath,
    fields: T,
}

impl<T: From<Keypath>> From<Keypath> for Node<T> {
    fn from(keypath: Keypath) -> Self {
        let fields = T::from(keypath.clone());
        Self { keypath, fields }
    }
}

impl<T> Node<T> {
    pub fn keypath(&self) -> &Keypath {
        &self.keypath
    }
}

impl<T> Deref for Node<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.fields
    }
}

impl<T> DerefMut for Node<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.fields
    }
}

impl<T> Sealed for Node<T> {}
impl<T: From<Keypath>> StorageType for Node<T> {}

impl<T: super::deletable::Deletable> super::deletable::Deletable for Node<T> {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        self.fields.delete_internal(flags);
    }
}

/// Marker for types usable as a contract's root state by `#[contract]`.
///
/// Automatically implemented for every type that implements
/// `From<Keypath>` — i.e. anything annotated with `#[storage]`, including
/// unit structs like `struct Token;`. Contract methods taking
/// `&self`/`&mut self` require the state type to implement this trait;
/// contracts with only associated functions never construct state and do
/// not need it.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used as contract state",
    note = "methods taking `&self`/`&mut self` require the state type to be a storage struct: add `#[storage]` to the definition of `{Self}`",
    note = "contracts with only associated functions (no `self` methods) do not need `#[storage]`"
)]
pub trait ContractState: From<Keypath> {}

impl<T: From<Keypath>> ContractState for T {}

/// Construct the root state node for a contract entrypoint.
///
/// Called by `#[contract]`-generated code; not intended for direct use.
#[doc(hidden)]
pub fn contract_state_root<T: ContractState>() -> Node<T> {
    Node::from(Keypath::root())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UnitState;
    impl From<Keypath> for UnitState {
        fn from(_: Keypath) -> Self {
            Self
        }
    }

    #[test]
    fn contract_state_root_creates_node() {
        let _node: Node<UnitState> = contract_state_root::<UnitState>();
    }
}
