use super::balance::Balance;
use super::bytestring::Bytestring;
use super::capacity_constraint::CapacityConstraint;
use super::keypath::Keypath;
use super::nnint::NNInt;
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
pub trait StorageType: Sealed + From<Keypath> {}

/// Marker for types usable as a field of a `#[storage]` struct.
///
/// Blanket-implemented for every type that implements `From<Keypath>`, so it
/// accepts exactly what the field initializers need and nothing more: the
/// storage handles, `Node<T>`, and nested `#[storage]` structs. It exists to
/// carry the diagnostic below — required directly, a plain field reports a
/// bare `From<Keypath>` bound at the derive's span, with rustc suggesting
/// `#![feature(trivial_bounds)]`, which is never the fix.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be a storage field",
    // Replaces rustc's default "the trait `X` is not implemented" label. The
    // trait name still reaches the user in the help lines rustc adds, but the
    // span annotation reads as guidance rather than naming an internal marker.
    label = "not a storage handle",
    // No list here either: the impls below are explicit and the `#[storage]`
    // derive adds one per struct, so rustc prints the set itself.
    note = "storage fields hold handles, not values: wrap plain data in `Value<{Self}>`"
)]
#[doc(hidden)]
pub trait __StorageFieldDiagnostic {
    /// Build the field handle from its keypath.
    ///
    /// The trait's only item, and `From<Keypath>` is deliberately *not* a
    /// supertrait: with one the failure of a non-storage field would be
    /// reported twice, once for `__StorageFieldDiagnostic` and once for the supertrait
    /// obligation behind it. The blanket impl below supplies this for every
    /// `From<Keypath>` type, so the set of accepted field types is the same
    /// either way. Not intended to be called or implemented directly.
    #[doc(hidden)]
    fn __from_keypath(keypath: Keypath) -> Self;
}

macro_rules! storage_field {
    ($($ty:ty),* $(,)?) => {$(
        impl __StorageFieldDiagnostic for $ty {
            fn __from_keypath(keypath: Keypath) -> Self {
                Self::from(keypath)
            }
        }
    )*};
}

storage_field!(NNInt, Balance, Bytestring, CapacityConstraint);

impl<T> __StorageFieldDiagnostic for super::value::Value<T> {
    fn __from_keypath(keypath: Keypath) -> Self {
        Self::from(keypath)
    }
}
impl<K, V> __StorageFieldDiagnostic for super::map::Map<K, V> {
    fn __from_keypath(keypath: Keypath) -> Self {
        Self::from(keypath)
    }
}
impl<S> __StorageFieldDiagnostic for super::vector::Vector<S> {
    fn __from_keypath(keypath: Keypath) -> Self {
        Self::from(keypath)
    }
}
impl<S> __StorageFieldDiagnostic for super::rcq::RCQ<S> {
    fn __from_keypath(keypath: Keypath) -> Self {
        Self::from(keypath)
    }
}
impl<T: From<Keypath>> __StorageFieldDiagnostic for Node<T> {
    fn __from_keypath(keypath: Keypath) -> Self {
        Self::from(keypath)
    }
}

/// Wrapper marking a compound storage field group.
///
/// Derefs to `T` for field access. The node does not retain its own keypath:
/// each field handle constructed by `T::from` already carries the path it
/// needs, so keeping the parent path would add an unused buffer and require an
/// otherwise-unnecessary clone during construction.
pub struct Node<T> {
    fields: T,
}

impl<T: From<Keypath>> From<Keypath> for Node<T> {
    fn from(keypath: Keypath) -> Self {
        Self {
            fields: T::from(keypath),
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
    Node::from(Keypath::root(T::DOMAIN))
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
