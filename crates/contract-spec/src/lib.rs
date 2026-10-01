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
    ContractSpec, ContractSpecId, ContractSpecInterface, ContractSpecMethod,
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
    /// The fully qualified id this type is known by in the contract spec.
    ///
    /// This is an identity claim, not a label. A bare ident cannot tell two
    /// user-defined types apart — two crates, two modules of one crate, or two
    /// major versions of one crate may each define a `Meta` — so every
    /// reference in a spec resolves through this const, and two types agree on
    /// an id only if they are the same type. The crate and major-version
    /// segments are load-bearing: shortening an id to its declared name for
    /// display is each codegen's business, not the spec's.
    ///
    /// The segments travel apart rather than joined into
    /// `CRATE@MAJOR::module::path::Name`, because the joined form cannot be
    /// split back: `name = "..."` substitutes its value verbatim, `::` and
    /// all, so the boundary between module path and declared name is not
    /// recoverable from the string.
    ///
    /// `#[contract_type]` and `#[contract_exception]` derive it at the
    /// declaration, where `module_path!()` and the declaring crate's name and
    /// version are all in scope, so it cannot be taken from the wrong crate. A
    /// hand-written impl (for a type this crate does not declare) is asserting
    /// the id itself and is responsible for its accuracy, segment by segment.
    const NAME: view::ContractSpecIdView<'static>;
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
//
// A slice is an unbounded vector of its element; for `[u8]` that is the
// byte-payload spelling above.
impl<T> ContractTypeRefSpec for [T]
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

/// Bounded vector with no allocator: the same wire shape and spec as
/// `VecM<T, N>` (for `u8`, as `BytesM<N>`), but decodable in a no-`alloc`
/// guest.
impl<T, const N: usize> ContractTypeRefSpec for heapless::Vec<T, N>
where
    T: ContractTypeRefSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Vector(view::ContractVectorTypeView {
            element: &type_ref::<T>(),
            max_len: N as u32,
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

/// Returns the spec id a `ContractTypeSpec` type names itself by.
///
/// The id, not a display name: read [`ContractSpecIdView::declared`] for the
/// bare identifier, and see that type on why the segments travel apart.
pub const fn symbol<T: ContractTypeSpec>() -> view::ContractSpecIdView<'static>
{
    T::TYPE_DEF.symbol()
}

/// Returns the spec type reference for `T` (e.g. `U32`, `Named("Foo")`).
pub const fn type_ref<T: ContractTypeRefSpec + ?Sized>()
-> view::ContractTypeView<'static> {
    T::INFO.ty
}
