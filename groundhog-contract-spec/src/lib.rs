#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::{string::String, vec::Vec};

pub mod view;

#[cfg(feature = "alloc")]
pub use groundhog_spec::{
    ContractArrayType, ContractEnumDef, ContractEnumVariant,
    ContractExceptionType, ContractField, ContractSpec, ContractSpecInterface,
    ContractSpecMethod, ContractSpecString, ContractStructDef, ContractType,
    ContractTypeDef, ContractTypeDefKind, ContractTypeKind, ContractUnionCase,
    ContractUnionCaseKind, ContractUnionCaseType, ContractUnionDef,
    ContractVectorType, SpecFragment, SpecFragmentKind, SpecInterfaceFragment,
    SpecMethodFragment,
};
pub use view::*;
#[cfg(feature = "alloc")]
use xdrs::VecM;

mod groundhog_xdr_types;

/// Implemented by `#[contracttype]` types to provide their spec type definition.
pub trait ContractTypeSpec {
    /// The type's definition (struct, enum, or union) as a const view.
    const TYPE_DEF: view::ContractTypeDefView<'static>;
}

/// Provides the spec-visible name for a contract type.
pub trait ContractTypeName {
    /// The name used to identify this type in the contract spec.
    const NAME: &'static str;
}

/// Bundles a type reference for use in contract spec signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractTypeInfo<'a> {
    /// The type reference (e.g. `U32`, `Named("Foo")`, `Option(&U64)`).
    pub ty: view::ContractTypeView<'a>,
}

/// Trait for types that can appear in contract spec signatures.
/// Implemented for primitives, collections, and `#[contracttype]` types.
pub trait ContractTypeRefSpec {
    /// The type reference and its transitive type definitions.
    const INFO: ContractTypeInfo<'static>;

    /// Hidden root symbol used to retain imported raw typedef fragments.
    #[doc(hidden)]
    const __GROUNDHOG_SPEC_ANCHOR_ROOT: fn() -> usize =
        Self::__groundhog_spec_anchor;

    /// Hidden linker-anchor hook used to retain raw typedef fragments.
    #[doc(hidden)]
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

impl ContractTypeRefSpec for [u8] {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Vector(view::ContractVectorTypeView {
            element: Some(&view::ContractTypeView::U8),
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

// XXX
impl<const MAX: u32> ContractTypeRefSpec for xdr_common::StringM<MAX> {
    const INFO: ContractTypeInfo<'static> = <str as ContractTypeRefSpec>::INFO;
}

impl<T, const N: usize> ContractTypeRefSpec for [T; N]
where
    T: ContractTypeRefSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Array(view::ContractArrayTypeView {
            element: Some(&type_ref::<T>()),
            len: N as u32,
        }),
    };

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
            element: Some(&type_ref::<T>()),
            max_len: u32::MAX,
        }),
    };

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
            element: Some(&type_ref::<T>()),
            max_len: N,
        }),
    };

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
        ty: view::ContractTypeView::Option(Some(&type_ref::<T>())),
    };

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
