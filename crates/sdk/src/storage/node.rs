use super::keypath::Keypath;
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
/// Called by `derive(Storage)`-generated `ContractState::const_root()` for
/// each field of a contract root struct. Not intended for direct use.
///
/// # Why this trait is not sealed
///
/// True sealing (via a private supertrait) is impossible because
/// `derive(Storage)` emits an implementation of this trait inside the *user's*
/// crate — a private supertrait defined in `sdk` would be unreachable there.
/// `#[doc(hidden)]` hides the trait from documentation; hand-written
/// implementations are technically allowed but receive no wire-compatibility
/// guarantee from the SDK.
///
/// # Field-type requirement
///
/// Every field of a `#[storage]` struct must implement this trait (the
/// derive bounds its `ContractState` impl on it, and the bound is checked
/// even for structs never used as a contract root). A type that only
/// implements `From<Keypath>` by hand therefore cannot appear as a
/// `#[storage]` field — mark the field type `#[storage]` itself, or use an
/// SDK storage type. The `on_unimplemented` diagnostic below points users
/// there.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be a field of a `#[storage]` struct",
    note = "fields of `#[storage]` structs must be SDK storage types \
            (`Value`, `NNInt`, `Map`, `Vector`, …) or structs that are \
            themselves `#[storage]`",
    note = "a hand-written `From<Keypath>` impl is not sufficient: add \
            `#[storage]` to the definition of `{Self}`"
)]
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
/// Not intended for direct use.
///
/// # Why this trait is not sealed
///
/// True sealing (via a private supertrait) is impossible because
/// `derive(Storage)` emits an implementation of this trait inside the *user's*
/// crate — a private supertrait defined in `sdk` would be unreachable there.
/// `#[doc(hidden)]` hides the trait from documentation; hand-written
/// implementations are technically allowed but receive no wire-compatibility
/// guarantee from the SDK.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used as contract state",
    note = "methods taking `&self`/`&mut self` require the state type to be a \
            storage struct: add `#[storage]` to the definition of `{Self}`",
    note = "contracts with only associated functions (no `self` methods) do \
            not need `#[storage]`"
)]
pub trait ContractState: StorageDomain {
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

/// Wrapper marking a compound storage field group; derefs to `T` for field
/// access.
///
/// The node materializes no path of its own — its field handles each carry
/// their full path (`Const` arm at the contract root, `Owned` below) — so
/// constructing a `Node` is free of keypath copies.
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
/// Uses the type's compile-time const path (from [`ContractState`]) to build
/// the whole skeleton with zero keypath copies: the `Node`'s own path is the
/// static domain frame; each direct leaf field is a `Const`-arm handle.
/// Called by `#[contract]`-generated code; not intended for direct use.
// #[inline(never)]: keep ONE shared skeleton builder per contract rather than
// duplicating the assembly into every method entrypoint. sret already places
// the returned Node directly in the caller's slot, so outlining adds no copy
// (the Const-arm skeleton still builds with zero keypath memcpys — verified on
// counter). Measured strictly smaller than #[inline(always)] across the fleet
// (storage_errors +272→+144, erc20 −35→−329, htlc −1102→−1198, counter
// −40→−56): the per-entrypoint duplication cost dominates the notional
// per-call inlining benefit.
#[doc(hidden)]
#[inline(never)]
pub fn contract_state_root<T: ContractState>() -> Node<T> {
    Node {
        fields: T::const_root(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::keypath::{FramedComponent, framed_domain_len};
    use crate::storage::{FramedDomain, frame_domain};

    // Use derive(Storage) so UnitState gets ContractState.
    #[derive(crate::Storage)]
    struct UnitState;

    #[test]
    fn unit_struct_contract_state_root_constructs() {
        let _node: Node<UnitState> = contract_state_root::<UnitState>();
    }

    // A named-field storage struct for drift-guard tests.
    #[derive(crate::Storage)]
    struct LeafState {
        value: crate::storage::value::Value<u32>,
    }

    /// Drift guard: the const-path `contract_state_root` produces the SAME
    /// leaf preimage as the runtime derivation path used today.
    ///
    /// Wire-safety requirement: if this fails, the const-path leaf addresses
    /// a DIFFERENT storage slot than the runtime path — a data-access bug.
    #[test]
    fn const_root_leaf_matches_runtime_derivation() {
        // Const path: built by contract_state_root via ContractState.
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

    // Slots >= 63 take multi-byte CXS index encodings (2-, 3-, and 5-byte
    // forms) — same index set as slot_placement.rs's runtime-path
    // `wide_slot_indices_match_runtime_child`, here exercised through the
    // CONST regime.
    #[derive(crate::Storage)]
    struct WideSlotState {
        #[slot(63)]
        a: crate::storage::value::Value<u32>,
        #[slot(300)]
        b: crate::storage::value::Value<u32>,
        #[slot(70_000)]
        c: crate::storage::value::Value<u32>,
        #[slot(4_294_967_295)]
        d: crate::storage::value::Value<u32>,
    }

    /// Drift guard for multi-byte CXS field indices in the CONST regime.
    ///
    /// The macro's const-path emission re-implements the field-frame codec
    /// (probe + ConstWriter); the other drift guards only pin index 0, whose
    /// frame is single-byte in every position. This test forces the emitted
    /// codec through the 1→2, 2→3, and maximal CXS width transitions and
    /// asserts byte identity with the runtime `child_framed` derivation —
    /// if the emitted probe/v_len logic ever mis-frames a wide index, a
    /// `#[slot(N)]` root field would silently address a different storage
    /// slot than prior SDK versions.
    #[test]
    fn const_root_wide_slot_indices_match_runtime_derivation() {
        let const_node: Node<WideSlotState> =
            contract_state_root::<WideSlotState>();

        const N_D: usize = framed_domain_len("WideSlotState");
        const DOMAIN: [u8; N_D] = frame_domain::<N_D>("WideSlotState");
        let root = Keypath::root(FramedDomain::new(&DOMAIN));

        const F_A: FramedComponent = FramedComponent::from_u32(63);
        const F_B: FramedComponent = FramedComponent::from_u32(300);
        const F_C: FramedComponent = FramedComponent::from_u32(70_000);
        const F_D: FramedComponent = FramedComponent::from_u32(u32::MAX);

        for (name, const_bytes, frame) in [
            ("a/63", const_node.a.path_bytes(), &F_A),
            ("b/300", const_node.b.path_bytes(), &F_B),
            ("c/70000", const_node.c.path_bytes(), &F_C),
            ("d/u32::MAX", const_node.d.path_bytes(), &F_D),
        ] {
            let runtime = root.child_framed(frame);
            assert_eq!(
                const_bytes,
                runtime.path_bytes(),
                "wide slot {name}: const-root preimage differs from runtime \
                 child_framed — CXS width drift in the macro codec"
            );
        }
    }
}
