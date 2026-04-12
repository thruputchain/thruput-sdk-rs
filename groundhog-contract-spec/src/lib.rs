#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::{string::String, vec::Vec};

pub mod view;

#[cfg(feature = "alloc")]
pub use groundhog_spec::{
    ContractArrayType, ContractEnumDef, ContractEnumVariant, ContractField,
    ContractSpec, ContractSpecInterface, ContractSpecMethod,
    ContractSpecString, ContractStructDef, ContractType, ContractTypeDef,
    ContractTypeDefKind, ContractTypeKind, ContractUnionCase,
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
    /// All type definitions this type transitively depends on, including itself.
    const TYPE_DEFS: view::ContractTypeDefsView<'static> =
        view::ContractTypeDefsView::One(Self::TYPE_DEF);
}

/// Provides the spec-visible name for a contract type.
pub trait ContractTypeName {
    /// The name used to identify this type in the contract spec.
    const NAME: &'static str;
}

/// Bundles a type reference with all transitive type definitions it depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractTypeInfo<'a> {
    /// The type reference (e.g. `U32`, `Named("Foo")`, `Option(&U64)`).
    pub ty: view::ContractTypeView<'a>,
    /// All type definitions reachable from `ty`.
    pub type_defs: view::ContractTypeDefsView<'a>,
}

/// Trait for types that can appear in contract spec signatures.
/// Implemented for primitives, collections, and `#[contracttype]` types.
pub trait ContractTypeRefSpec {
    /// The type reference and its transitive type definitions.
    const INFO: ContractTypeInfo<'static>;
}

impl ContractTypeRefSpec for () {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Void,
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for bool {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Bool,
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for u8 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U8,
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for u32 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U32,
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for i32 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I32,
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for u64 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U64,
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for i64 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I64,
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for u128 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U128,
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for i128 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I128,
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for str {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::String,
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for [u8] {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Vector(view::ContractVectorTypeView {
            element: Some(&view::ContractTypeView::U8),
            max_len: u32::MAX,
        }),
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for &[u8] {
    const INFO: ContractTypeInfo<'static> =
        <[u8] as ContractTypeRefSpec>::INFO;
}

impl ContractTypeRefSpec for &str {
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
        type_defs: type_defs::<T>(),
    };
}

#[cfg(feature = "alloc")]
impl ContractTypeRefSpec for String {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::String,
        type_defs: view::ContractTypeDefsView::Empty,
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
        type_defs: type_defs::<T>(),
    };
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
        type_defs: type_defs::<T>(),
    };
}

impl<T> ContractTypeRefSpec for Option<T>
where
    T: ContractTypeRefSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Option(Some(&type_ref::<T>())),
        type_defs: type_defs::<T>(),
    };
}

impl<T> ContractTypeRefSpec for T
where
    T: ContractTypeSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: T::TYPE_DEF.type_ref(),
        type_defs: T::TYPE_DEFS,
    };
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
                    type_defs: view::ContractTypeDefsView::Many(&[$(type_defs::<$name>()),+]),
                };
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

/// Returns all transitive type definitions that `T` depends on.
pub const fn type_defs<T: ContractTypeRefSpec + ?Sized>()
-> view::ContractTypeDefsView<'static> {
    T::INFO.type_defs
}

/// Computes the XDR-serialized byte length for a tree of type-def spec fragments.
pub const fn const_txs_typedef_fragment_closure_len(
    type_defs: view::ContractTypeDefsView<'_>,
) -> usize {
    match type_defs {
        view::ContractTypeDefsView::Empty => 0,
        view::ContractTypeDefsView::One(type_def) => {
            view::SpecFragmentView::TypeDef(type_def).const_txs_size()
        }
        view::ContractTypeDefsView::Many(type_defs) => {
            let mut total = 0usize;
            let mut i = 0usize;
            while i < type_defs.len() {
                total += const_txs_typedef_fragment_closure_len(type_defs[i]);
                i += 1;
            }
            total
        }
    }
}

const fn const_write_txs_typedef_fragment_closure<const OUT: usize>(
    w: &mut xdrs::txs::ConstWriter<OUT>,
    type_defs: view::ContractTypeDefsView<'_>,
) {
    match type_defs {
        view::ContractTypeDefsView::Empty => {}
        view::ContractTypeDefsView::One(type_def) => {
            view::SpecFragmentView::TypeDef(type_def).const_write_txs(w);
        }
        view::ContractTypeDefsView::Many(type_defs) => {
            let mut i = 0usize;
            while i < type_defs.len() {
                const_write_txs_typedef_fragment_closure(w, type_defs[i]);
                i += 1;
            }
        }
    }
}

/// Serializes a tree of type-def spec fragments into a fixed-size byte array at compile time.
pub const fn const_txs_typedef_fragment_closure_bytes<const OUT: usize>(
    type_defs: view::ContractTypeDefsView<'_>,
) -> [u8; OUT] {
    assert!(OUT == const_txs_typedef_fragment_closure_len(type_defs));
    let mut w = xdrs::txs::ConstWriter::<OUT>::new();
    const_write_txs_typedef_fragment_closure(&mut w, type_defs);
    w.into_bytes()
}
