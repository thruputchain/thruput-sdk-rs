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

/// Carries a storage struct's root domain separator.
///
/// Derived by `#[storage]` / `#[derive(Storage)]`. `DOMAIN` defaults to the
/// struct's bare identifier and is overridable with `#[storage(domain = "…")]`.
/// Only consulted when the type is used as a contract-state root
/// (see [`contract_state_root`]); nested usage ignores it.
pub trait StorageDomain {
    const DOMAIN: &'static str;
    /// [`Self::DOMAIN`] pre-framed at compile time (see
    /// [`frame_domain`](crate::storage::frame_domain)); consumed by
    /// [`contract_state_root`]. Emitted by `derive(Storage)`; a hand-written
    /// impl must keep the two consts consistent.
    #[doc(hidden)]
    const DOMAIN_FRAMED: crate::storage::FramedDomain;
}

/// Construct the root state node for a contract entrypoint.
///
/// Seeds the keypath with the type's [`StorageDomain::DOMAIN`] so distinct
/// state roots linked into one artifact occupy disjoint subtrees.
/// Called by `#[contract]`-generated code; not intended for direct use.
// Root construction zeroes a STATIC_KEYPATH_CAPACITY-byte buffer (via
// `Keypath::root`); the body is now small enough that LLVM inlines it into
// every entrypoint, duplicating that zeroing/copy at each call site
// (measured +1.2KB code size on erc20's 10 entrypoints). Keep it out of line.
#[doc(hidden)]
#[inline(never)]
pub fn contract_state_root<T: ContractState + StorageDomain>() -> Node<T> {
    Node::from(Keypath::root(T::DOMAIN_FRAMED))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageDomain;
    use crate::storage::{FramedDomain, frame_domain, framed_domain_len};

    struct UnitState;
    impl From<Keypath> for UnitState {
        fn from(_: Keypath) -> Self {
            Self
        }
    }
    impl StorageDomain for UnitState {
        const DOMAIN: &'static str = "UnitState";
        const DOMAIN_FRAMED: FramedDomain = {
            const N: usize = framed_domain_len("UnitState");
            const BYTES: [u8; N] = frame_domain::<N>("UnitState");
            FramedDomain::new(&BYTES)
        };
    }

    #[test]
    fn contract_state_root_seeds_domain() {
        let node: Node<UnitState> = contract_state_root::<UnitState>();
        // The const-framed seed equals the runtime child() frame exactly.
        let expected = Keypath::default().child("UnitState");
        assert!(node.keypath().addresses_same_slot(&expected));
        // And it is NOT the bare empty root.
        assert!(!node.keypath().addresses_same_slot(&Keypath::default()));
    }
}
