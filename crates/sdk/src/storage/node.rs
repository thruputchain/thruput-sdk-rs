use super::keypath::Keypath;
use super::sealed::Sealed;
use core::ops::{Deref, DerefMut};
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// Marker trait for types that can appear as elements in Map and Vector.
/// Sealed — only SDK storage types implement this.
#[allow(private_bounds)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be stored in a `Map` or `Vector`",
    // No list of handle types here: the trait is sealed with explicit impls,
    // so rustc enumerates them itself. A hand-kept copy only drifts.
    note = "element types must be storage handles: wrap plain data in `Value<{Self}>`; a nested `#[storage]` struct goes in `Node<_>`"
)]
pub trait StorageType: Sealed + StorageHandle {}

/// Builds a storage handle from the keypath its slot resolves to.
///
/// The constructor every storage handle is made through, and the bound the
/// `#[storage]` derive puts on its fields. Implemented by the SDK handle
/// types and, via the derive, by every `#[storage]` struct; plain data is
/// wrapped in [`Value<T>`](super::Value) rather than implementing this.
///
/// Replaces what used to be a `From<Keypath>` bound. Owning the trait is what
/// lets the derive's error say which field is wrong and what to do about it —
/// a `#[diagnostic::on_unimplemented]` cannot be attached to `From`.
///
/// # Migrating a hand-rolled handle
///
/// The derive used to bound fields on `From<Keypath>`, so a type implementing
/// that by hand was accepted as a storage field. Such a type now needs a
/// `StorageHandle` impl too, which can forward to the existing conversion:
///
/// ```ignore
/// impl StorageHandle for MyHandle {
///     fn from_keypath(keypath: Keypath) -> Self {
///         Self::from(keypath)
///     }
/// }
/// ```
///
/// The SDK handle types and every `#[storage]` struct are covered already, and
/// nothing in this repo or `example-contracts` writes such an impl by hand.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be a storage field",
    // No hand-kept list of handle types: the impls are explicit and the
    // derive adds one per struct, so rustc enumerates the set itself.
    note = "storage fields hold handles, not values: wrap plain data in `Value<{Self}>`"
)]
pub trait StorageHandle {
    /// Build the handle for `keypath`.
    fn from_keypath(keypath: Keypath) -> Self;
}

/// Wrapper marking a compound storage field group.
///
/// Derefs to `T` for field access. The node does not retain its own keypath:
/// each field handle constructed by `T::from_keypath` already carries the path it
/// needs, so keeping the parent path would add an unused buffer and require an
/// otherwise-unnecessary clone during construction.
pub struct Node<T> {
    fields: T,
}

impl<T: StorageHandle> StorageHandle for Node<T> {
    fn from_keypath(keypath: Keypath) -> Self {
        Self {
            fields: T::from_keypath(keypath),
        }
    }
}

// Convenience: keeps `keypath.into()` working for nodes too.
impl<T: StorageHandle> From<Keypath> for Node<T> {
    fn from(keypath: Keypath) -> Self {
        <Self as StorageHandle>::from_keypath(keypath)
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
impl<T: StorageHandle> StorageType for Node<T> {}

impl<T: super::deletable::Deletable> super::deletable::Deletable for Node<T> {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        self.fields.delete_internal(flags);
    }
}

/// Marker for types usable as a contract's root state by `#[contract]`.
///
/// Automatically implemented for every type that implements
/// `StorageHandle` — i.e. anything annotated with `#[storage]`, including
/// unit structs like `struct Token;`. Contract methods taking
/// `&self`/`&mut self` require the state type to implement this trait;
/// contracts with only associated functions never construct state and do
/// not need it.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used as contract state",
    note = "methods taking `&self`/`&mut self` require the state type to be a storage struct: add `#[storage]` to the definition of `{Self}`",
    note = "contracts with only associated functions (no `self` methods) do not need `#[storage]`"
)]
pub trait ContractState: StorageHandle {}

impl<T: StorageHandle> ContractState for T {}

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
/// Seeds the keypath with the type's [`StorageDomain::DOMAIN`] so distinct
/// state roots linked into one artifact occupy disjoint subtrees.
/// Called by `#[contract]`-generated code; not intended for direct use.
// Still measurably worth keeping out of line even after `StaticKeyBuf`
// dropped its zero-fill: the body is small enough that LLVM inlines it into
// every entrypoint, duplicating the CXS-framing/copy work at each call site.
// Measured on erc20 (10 entrypoints, optimized release build, code size):
// inline(never) = 14583 bytes, inlined = 16118 bytes (+1535 bytes). Keep out
// of line.
#[doc(hidden)]
#[inline(never)]
pub fn contract_state_root<T: ContractState + StorageDomain>() -> Node<T> {
    Node::from_keypath(Keypath::root(T::DOMAIN))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(crate::Storage)]
    struct UnitState;

    crate::contract_test!(unit_struct_contract_state_root_constructs {
        let _node: Node<UnitState> = contract_state_root::<UnitState>();
    });

    #[derive(crate::Storage)]
    struct LeafState {
        value: crate::storage::Value<u64>,
    }

    crate::contract_test!(contract_state_root_seeds_domain {
        let node: Node<LeafState> = contract_state_root::<LeafState>();
        // Observe the domain through a real field path: the Node itself does
        // not retain a redundant copy of the parent path.
        let expected = Keypath::default().child("LeafState").child(&0u32);
        assert_eq!(*node.value.key(), expected.generate_key());
    });
}
