#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::{string::String, vec::Vec};

pub mod format;
pub mod view;

pub use format::{
    FINAL_SECTION_NAME, RAW_SECTION_NAME, RAW_SECTION_PREFIX, RAW_SPEC_FORMAT,
    SpecSection, classify_section,
};

#[cfg(feature = "owned-model")]
pub use groundhog_spec::{
    ContractArrayType, ContractEnumDef, ContractEnumVariant, ContractEventDef,
    ContractExceptionType, ContractField, ContractSignedTokenPreimage,
    ContractSpec, ContractSpecInterface, ContractSpecMethod,
    ContractSpecString, ContractStructDef, ContractType, ContractTypeDef,
    ContractTypeDefKind, ContractTypeKind, ContractUnionCase,
    ContractUnionCaseKind, ContractUnionCaseType, ContractUnionDef,
    ContractVectorType,
};
#[cfg(feature = "owned-model")]
mod fragment;
#[cfg(feature = "owned-model")]
pub use fragment::*;
pub use view::*;
#[cfg(feature = "alloc")]
use xdrs::VecM;

mod groundhog_xdr_types;

/// Implemented by `#[contract_type]` types to provide their spec type definition.
pub trait ContractTypeSpec {
    /// The type's definition (struct, enum, or union) as a const view.
    const TYPE_DEF: view::ContractTypeDefView<'static>;
}

/// Implemented by `#[derive(Event)]` types to provide their spec event
/// definition. The definition is emitted into the raw spec section only
/// where the event is declared (`emits(...)` on an interface) or exported
/// (`#[event_spec(export)]` on the derive).
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a contract event",
    label = "not a contract event",
    note = "add `#[derive(Event)]` to `{Self}` — `emits(...)` in \
            `#[contract(...)]` requires it"
)]
pub trait ContractEventSpec {
    /// The event's definition (name, signature, topics, fields) as a const view.
    const EVENT_DEF: view::ContractEventDefView<'static>;

    /// Hidden root symbol used to retain raw typedef fragments for the
    /// event's field types.
    #[doc(hidden)]
    const __GROUNDHOG_SPEC_ANCHOR_ROOT: fn() -> usize =
        Self::__groundhog_spec_anchor;

    /// Hidden linker-anchor hook used to retain raw typedef fragments.
    #[doc(hidden)]
    #[cfg_attr(target_arch = "riscv32", inline(never))]
    #[cfg_attr(
        target_arch = "riscv32",
        unsafe(link_section = "groundhogspecanchortextv0")
    )]
    fn __groundhog_spec_anchor() -> usize {
        0
    }
}

/// Provides the spec-visible name for a contract type.
pub trait ContractTypeName {
    /// The fully qualified id this type is known by in the contract spec:
    /// `CRATE@MAJOR::module::path::Name`.
    ///
    /// This is an identity claim, not a label. A bare ident cannot tell two
    /// user-defined types apart — two crates, two modules of one crate, or two
    /// major versions of one crate may each define a `Meta` — so every
    /// reference in a spec resolves through this const, and two types agree on
    /// an id only if they are the same type. The crate and major-version
    /// segments are load-bearing: shortening the id to its final segment for
    /// display is each codegen's business, not the spec's.
    ///
    /// `#[contract_type]` and `#[contract_exception]` derive it at the
    /// declaration, where `module_path!()` and the declaring crate's version
    /// are both in scope, so it cannot be taken from the wrong crate. A
    /// hand-written impl (for a type this crate does not declare) is asserting
    /// the id itself and is responsible for its accuracy; prefer
    /// [`qualified_id_len`]/[`qualified_id_bytes`] over a literal so the crate
    /// and version are still filled in mechanically.
    const NAME: &'static str;
}

/// Byte length of the id [`qualified_id_bytes`] builds, for sizing its array.
pub const fn qualified_id_len(
    module_path: &str,
    major: &str,
    declared: &str,
) -> usize {
    // One `@` inserted after the crate segment, one `::` before the name.
    module_path.len() + 1 + major.len() + 2 + declared.len()
}

/// Builds `CRATE@MAJOR::module::path::Declared` from `module_path!()` by
/// inserting `@MAJOR` after the crate segment and appending the declared name.
///
/// The crate segment is taken from the module path rather than from
/// `CARGO_PKG_NAME` so that one source decides it, spelled the one way Rust
/// spells it (underscores, not hyphens).
///
/// `N` must be [`qualified_id_len`] of the same three inputs.
pub const fn qualified_id_bytes<const N: usize>(
    module_path: &str,
    major: &str,
    declared: &str,
) -> [u8; N] {
    let path = module_path.as_bytes();
    let major = major.as_bytes();
    let declared = declared.as_bytes();
    let mut out = [0u8; N];
    let mut w = 0;

    // The crate segment is everything before the first `::`; a type declared
    // in the crate root has no `::` and the whole path is the crate segment.
    let mut root = path.len();
    let mut i = 0;
    while i + 1 < path.len() {
        if path[i] == b':' && path[i + 1] == b':' {
            root = i;
            break;
        }
        i += 1;
    }

    let mut i = 0;
    while i < root {
        out[w] = path[i];
        w += 1;
        i += 1;
    }
    out[w] = b'@';
    w += 1;
    let mut i = 0;
    while i < major.len() {
        out[w] = major[i];
        w += 1;
        i += 1;
    }
    // The rest of the module path, which already carries its leading `::`.
    let mut i = root;
    while i < path.len() {
        out[w] = path[i];
        w += 1;
        i += 1;
    }
    out[w] = b':';
    w += 1;
    out[w] = b':';
    w += 1;
    let mut i = 0;
    while i < declared.len() {
        out[w] = declared[i];
        w += 1;
        i += 1;
    }

    out
}

/// Bundles a type reference for use in contract spec signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractTypeInfo<'a> {
    /// The type reference (e.g. `U32`, `Named("Foo")`, `Option(&U64)`).
    pub ty: view::ContractTypeView<'a>,
}

/// Trait for types that can appear in contract spec signatures.
/// Implemented for primitives, collections, and `#[contract_type]` types.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot appear in a contract signature",
    label = "not a contract-spec type",
    note = "annotate the defining type with `#[contract_type]` (error \
            types: `#[contract_exception]`, events: `#[derive(Event)]`)"
)]
pub trait ContractTypeRefSpec {
    /// The type reference. Transitive type definitions are not carried here;
    /// they are retained via the linker-anchor mechanism
    /// (`__GROUNDHOG_SPEC_ANCHOR_ROOT`) and resolved by spec-tool.
    const INFO: ContractTypeInfo<'static>;

    /// How this type appears as a signed-token preimage
    /// (`ContractTypeView::SignedToken`'s payload): the type itself by
    /// default, or `None` for a type-erased payload (the SDK's `Opaque`
    /// overrides this).
    const SIGNED_PREIMAGE: Option<&'static view::ContractTypeView<'static>> =
        Some(&Self::INFO.ty);

    /// Hidden root symbol used to retain imported raw typedef fragments.
    #[doc(hidden)]
    const __GROUNDHOG_SPEC_ANCHOR_ROOT: fn() -> usize =
        Self::__groundhog_spec_anchor;

    /// Hidden linker-anchor hook used to retain raw typedef fragments.
    #[doc(hidden)]
    #[cfg_attr(target_arch = "riscv32", inline(never))]
    #[cfg_attr(
        target_arch = "riscv32",
        unsafe(link_section = "groundhogspecanchortextv0")
    )]
    fn __groundhog_spec_anchor() -> usize {
        0
    }
}

#[doc(hidden)]
#[inline(always)]
pub fn mix_anchor_fn(current: usize, anchor: fn() -> usize) -> usize {
    current ^ core::hint::black_box(anchor as usize)
}

impl ContractTypeRefSpec for () {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Void,
    };
}

impl ContractTypeRefSpec for bool {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Bool,
    };
}

impl ContractTypeRefSpec for u8 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U8,
    };
}

impl ContractTypeRefSpec for u32 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U32,
    };
}

impl ContractTypeRefSpec for i32 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I32,
    };
}

impl ContractTypeRefSpec for u64 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U64,
    };
}

impl ContractTypeRefSpec for i64 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I64,
    };
}

impl ContractTypeRefSpec for u128 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U128,
    };
}

impl ContractTypeRefSpec for i128 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I128,
    };
}

impl ContractTypeRefSpec for str {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::String,
    };
}

// Byte payloads deliberately lower to `Vector(U8, ...)`, never to
// `ContractType::Bytes`, so the spec has a single canonical spelling for
// byte sequences (the macro tests assert `Bytes` never appears). The
// `Bytes` wire variant still exists; other-language SDKs must also avoid
// emitting it, or identical interfaces will hash/compare unequal.
impl ContractTypeRefSpec for [u8] {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Vector(view::ContractVectorTypeView {
            element: &view::ContractTypeView::U8,
            max_len: u32::MAX,
        }),
    };
}

impl ContractTypeRefSpec for &[u8] {
    const INFO: ContractTypeInfo<'static> =
        <[u8] as ContractTypeRefSpec>::INFO;
}

impl ContractTypeRefSpec for &str {
    const INFO: ContractTypeInfo<'static> = <str as ContractTypeRefSpec>::INFO;
}

impl<const MAX: u32> ContractTypeRefSpec for xdr_common::StringM<MAX> {
    const INFO: ContractTypeInfo<'static> = <str as ContractTypeRefSpec>::INFO;
}

impl<const MAX: usize> ContractTypeRefSpec for heapless::String<MAX> {
    const INFO: ContractTypeInfo<'static> = <str as ContractTypeRefSpec>::INFO;
}

impl<const MAX: u32> ContractTypeRefSpec for xdr_common::BytesM<MAX> {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Vector(view::ContractVectorTypeView {
            element: &view::ContractTypeView::U8,
            max_len: MAX,
        }),
    };
}

/// Bounded bytes with no allocator: the same wire shape as `BytesM<N>`, and
/// the same spec, but decodable in a no-`alloc` guest.
impl<const N: usize> ContractTypeRefSpec for heapless::Vec<u8, N> {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Vector(view::ContractVectorTypeView {
            element: &view::ContractTypeView::U8,
            max_len: N as u32,
        }),
    };
}

impl<T, const N: usize> ContractTypeRefSpec for [T; N]
where
    T: ContractTypeRefSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Array(view::ContractArrayTypeView {
            element: &type_ref::<T>(),
            len: N as u32,
        }),
    };

    #[cfg_attr(target_arch = "riscv32", inline(never))]
    #[cfg_attr(
        target_arch = "riscv32",
        unsafe(link_section = "groundhogspecanchortextv0")
    )]
    fn __groundhog_spec_anchor() -> usize {
        mix_anchor_fn(
            0,
            <T as ContractTypeRefSpec>::__GROUNDHOG_SPEC_ANCHOR_ROOT,
        )
    }
}

#[cfg(feature = "alloc")]
impl ContractTypeRefSpec for String {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::String,
    };
}

#[cfg(feature = "alloc")]
impl<T> ContractTypeRefSpec for Vec<T>
where
    T: ContractTypeRefSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Vector(view::ContractVectorTypeView {
            element: &type_ref::<T>(),
            max_len: u32::MAX,
        }),
    };

    #[cfg_attr(target_arch = "riscv32", inline(never))]
    #[cfg_attr(
        target_arch = "riscv32",
        unsafe(link_section = "groundhogspecanchortextv0")
    )]
    fn __groundhog_spec_anchor() -> usize {
        mix_anchor_fn(
            0,
            <T as ContractTypeRefSpec>::__GROUNDHOG_SPEC_ANCHOR_ROOT,
        )
    }
}

#[cfg(feature = "alloc")]
impl<T, const N: u32> ContractTypeRefSpec for VecM<T, N>
where
    T: ContractTypeRefSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Vector(view::ContractVectorTypeView {
            element: &type_ref::<T>(),
            max_len: N,
        }),
    };

    #[cfg_attr(target_arch = "riscv32", inline(never))]
    #[cfg_attr(
        target_arch = "riscv32",
        unsafe(link_section = "groundhogspecanchortextv0")
    )]
    fn __groundhog_spec_anchor() -> usize {
        mix_anchor_fn(
            0,
            <T as ContractTypeRefSpec>::__GROUNDHOG_SPEC_ANCHOR_ROOT,
        )
    }
}

impl<T> ContractTypeRefSpec for Option<T>
where
    T: ContractTypeRefSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Option(&type_ref::<T>()),
    };

    #[cfg_attr(target_arch = "riscv32", inline(never))]
    #[cfg_attr(
        target_arch = "riscv32",
        unsafe(link_section = "groundhogspecanchortextv0")
    )]
    fn __groundhog_spec_anchor() -> usize {
        mix_anchor_fn(
            0,
            <T as ContractTypeRefSpec>::__GROUNDHOG_SPEC_ANCHOR_ROOT,
        )
    }
}

#[cfg(feature = "alloc")]
impl<T> ContractTypeRefSpec for alloc::boxed::Box<T>
where
    T: ContractTypeRefSpec,
{
    const INFO: ContractTypeInfo<'static> = T::INFO;

    #[cfg_attr(target_arch = "riscv32", inline(never))]
    #[cfg_attr(
        target_arch = "riscv32",
        unsafe(link_section = "groundhogspecanchortextv0")
    )]
    fn __groundhog_spec_anchor() -> usize {
        mix_anchor_fn(
            0,
            <T as ContractTypeRefSpec>::__GROUNDHOG_SPEC_ANCHOR_ROOT,
        )
    }
}

macro_rules! impl_tuple_type_ref_spec {
    ($(($($name:ident),+)),+ $(,)?) => {
        $(
            impl<$($name),+> ContractTypeRefSpec for ($($name,)+)
            where
                $($name: ContractTypeRefSpec),+
            {
                const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
                    ty: view::ContractTypeView::Tuple(&[$(type_ref::<$name>()),+]),
                };

                #[cfg_attr(target_arch = "riscv32", inline(never))]
                #[cfg_attr(
                    target_arch = "riscv32",
                    unsafe(link_section = "groundhogspecanchortextv0")
                )]
                fn __groundhog_spec_anchor() -> usize {
                    let mut acc = 0usize;
                    $(acc = mix_anchor_fn(acc, <$name as ContractTypeRefSpec>::__GROUNDHOG_SPEC_ANCHOR_ROOT);)+
                    acc
                }
            }
        )+
    };
}

impl_tuple_type_ref_spec!(
    (A),
    (A, B),
    (A, B, C),
    (A, B, C, D),
    (A, B, C, D, E),
    (A, B, C, D, E, F),
    (A, B, C, D, E, F, G),
    (A, B, C, D, E, F, G, H),
    (A, B, C, D, E, F, G, H, I),
    (A, B, C, D, E, F, G, H, I, J),
    (A, B, C, D, E, F, G, H, I, J, K),
    (A, B, C, D, E, F, G, H, I, J, K, L),
    (A, B, C, D, E, F, G, H, I, J, K, L, M),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P),
);

/// Returns the spec-visible symbol name for a `ContractTypeSpec` type.
pub const fn symbol<T: ContractTypeSpec>() -> &'static str {
    T::TYPE_DEF.symbol()
}

/// Returns the spec type reference for `T` (e.g. `U32`, `Named("Foo")`).
pub const fn type_ref<T: ContractTypeRefSpec + ?Sized>()
-> view::ContractTypeView<'static> {
    T::INFO.ty
}
