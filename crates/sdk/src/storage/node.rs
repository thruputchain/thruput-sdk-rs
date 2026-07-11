use super::keypath::{KeyCache, Keypath};
use super::sealed::Sealed;
use core::ops::{Deref, DerefMut};
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// Types constructible from a `'static` const preimage path slice.
///
/// Implemented by all SDK storage leaf types (`Value`, `NNInt`, `Bytestring`,
/// `RCQ`, `Map`) using `KeyCache::from_const` — zero memcpy construction.
/// `Vector` and `Node<T>` fall back to `Keypath::from_bytes` (one copy).
/// `derive(Storage)`-generated code implements it for user structs (also via
/// the `Keypath::from_bytes` fallback), so user-defined storage types can
/// appear as direct fields of a contract root.
///
/// Called by `derive(Storage)`-generated `ConstRootState::const_root()` for
/// each field of a contract root struct. Not intended for direct use.
#[doc(hidden)]
pub trait FromConstPath {
    fn __sdk_from_const_path(path: &'static [u8]) -> Self;
}

impl<T: From<Keypath>> FromConstPath for Node<T> {
    /// v1 depth-1 rule: a `Node<T>` direct field of the contract root
    /// gets ONE owned Keypath built from the const prefix, then delegates
    /// to today's `From<Keypath>` machinery for the subtree.
    fn __sdk_from_const_path(path: &'static [u8]) -> Self {
        Node::from(Keypath::from_bytes(path))
    }
}

/// Compile-time const-path root skeleton for `derive(Storage)` structs.
///
/// Implemented by `derive(Storage)` for all named-field and unit structs.
/// Enables `contract_state_root` to build the whole root skeleton with zero
/// keypath copies: each direct leaf field gets a `Const`-arm handle carrying
/// a `'static` preimage slice assembled at compile time.
///
/// The const path bytes `ROOT_PATH` are the type's storage domain frame —
/// identical to `StorageDomain::DOMAIN.as_bytes()` but exposed as a
/// `'static` slice for `Node::from_const_parts`.
///
/// Not intended for direct use.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used as contract state",
    note = "methods taking `&self`/`&mut self` require the state type to be a \
            storage struct: add `#[storage]` to the definition of `{Self}`",
    note = "contracts with only associated functions (no `self` methods) do \
            not need `#[storage]`"
)]
pub trait ConstRootState: StorageDomain {
    /// Static preimage bytes of this type's storage root (the domain frame).
    /// `sha256` of these bytes is the `Node`'s own storage key.
    const ROOT_PATH: &'static [u8];

    /// Build a `Self` where each direct leaf field is a `Const`-arm handle
    /// pointing at a `'static` preimage assembled at compile time.
    fn const_root() -> Self;
}

/// Marker trait for types that can appear as elements in Map and Vector.
/// Sealed — only SDK storage types implement this.
#[allow(private_bounds)]
pub trait StorageType: Sealed + From<Keypath> {}

/// A keypath-storing wrapper for compound storage field groups.
///
/// Pairs a `T: From<Keypath>` with its root keypath.
/// Derefs to T for field access.
pub struct Node<T> {
    key: KeyCache,
    fields: T,
}

impl<T: From<Keypath>> From<Keypath> for Node<T> {
    fn from(keypath: Keypath) -> Self {
        let fields = T::from(keypath.clone());
        Self {
            key: KeyCache::from(keypath),
            fields,
        }
    }
}

impl<T> Node<T> {
    /// Raw preimage bytes of this node's storage path.
    ///
    /// Useful for path-identity comparisons (e.g. drift-guard tests).
    pub fn path_bytes(&self) -> &[u8] {
        self.key.path_bytes()
    }

    /// Construct a `Node<T>` from a const-path byte slice and a pre-built
    /// fields value.
    ///
    /// Called by `#[contract]`-generated code in Task 3 to assemble the
    /// root skeleton without materializing any owned `Keypath` buffers.
    /// The `path` slice must be a `'static` preimage assembled at compile
    /// time (see `KeyCache::from_const`).
    pub fn from_const_parts(path: &'static [u8], fields: T) -> Node<T> {
        Node {
            key: KeyCache::from_const(path),
            fields,
        }
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
    /// The type's storage domain, pre-framed at compile time (see
    /// [`frame_domain`](crate::storage::frame_domain)); consumed by
    /// [`contract_state_root`]. Emitted by `derive(Storage)`.
    const DOMAIN: crate::storage::FramedDomain;
}

/// Construct the root state node for a contract entrypoint.
///
/// Uses the type's compile-time const path (from [`ConstRootState`]) to build
/// the whole skeleton with zero keypath copies: the `Node`'s own path is the
/// static domain frame; each direct leaf field is a `Const`-arm handle.
/// Called by `#[contract]`-generated code; not intended for direct use.
// Still measurably worth keeping out of line: see the inline(never) note on
// the previous implementation.  The body is now smaller, but LLVM may still
// choose to duplicate it per entrypoint.
#[doc(hidden)]
#[inline(never)]
pub fn contract_state_root<T: ConstRootState>() -> Node<T> {
    Node::from_const_parts(T::ROOT_PATH, T::const_root())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::keypath::{FramedComponent, framed_domain_len};
    use crate::storage::{FramedDomain, frame_domain};

    // Use derive(Storage) so UnitState gets ConstRootState.
    #[derive(crate::Storage)]
    struct UnitState;

    // A named-field storage struct for drift-guard tests.
    #[derive(crate::Storage)]
    struct LeafState {
        value: crate::storage::value::Value<u32>,
    }

    #[test]
    fn contract_state_root_seeds_domain() {
        let node: Node<UnitState> = contract_state_root::<UnitState>();
        // The const-framed seed equals the runtime child() frame exactly.
        let expected = Keypath::default().child("UnitState");
        assert_eq!(node.path_bytes(), expected.path_bytes());
        // And it is NOT the bare empty root.
        assert_ne!(node.path_bytes(), Keypath::default().path_bytes());
    }

    /// Drift guard: the const-path `contract_state_root` produces the SAME
    /// leaf preimage as the runtime derivation path used today.
    ///
    /// Wire-safety requirement: if this fails, the const-path leaf addresses
    /// a DIFFERENT storage slot than the runtime path — a data-access bug.
    #[test]
    fn const_root_leaf_matches_runtime_derivation() {
        // Const path: built by contract_state_root via ConstRootState.
        let const_node: Node<LeafState> = contract_state_root::<LeafState>();
        let const_leaf_bytes = const_node.value.path_bytes();

        // Runtime path: what today's From<Keypath> would produce.
        const N_D: usize = framed_domain_len("LeafState");
        const DOMAIN: [u8; N_D] = frame_domain::<N_D>("LeafState");
        const F0: FramedComponent = FramedComponent::from_u32(0u32);
        let runtime_kp =
            Keypath::root(FramedDomain::new(&DOMAIN)).child_framed(&F0);
        let runtime_bytes = runtime_kp.path_bytes();

        assert_eq!(
            const_leaf_bytes, runtime_bytes,
            "const-path leaf preimage differs from runtime path — drift detected"
        );
    }

    /// Drift guard (golden vector): the const-root path_bytes equal the
    /// domain frame, same as the runtime Keypath::root path.
    #[test]
    fn const_root_node_path_matches_runtime_root() {
        let const_node: Node<LeafState> = contract_state_root::<LeafState>();

        const N_D: usize = framed_domain_len("LeafState");
        const DOMAIN: [u8; N_D] = frame_domain::<N_D>("LeafState");
        let runtime_root = Keypath::root(FramedDomain::new(&DOMAIN));

        assert_eq!(const_node.path_bytes(), runtime_root.path_bytes());
    }
}
