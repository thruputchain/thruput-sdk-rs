#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::{string::String, vec::Vec};

#[cfg(feature = "alloc")]
pub mod model;
pub mod view;

#[cfg(feature = "alloc")]
pub use model::*;
pub use view::*;

pub trait ContractTypeSpec {
    const TYPE_DEF: view::ContractTypeDefView<'static>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractTypeInfo<'a> {
    pub ty: view::ContractTypeView<'a>,
    pub type_def: Option<view::ContractTypeDefView<'a>>,
}

pub trait ContractTypeRefSpec {
    const INFO: ContractTypeInfo<'static>;
}

impl ContractTypeRefSpec for () {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Void,
        type_def: None,
    };
}

impl ContractTypeRefSpec for bool {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Bool,
        type_def: None,
    };
}

impl ContractTypeRefSpec for u32 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U32,
        type_def: None,
    };
}

impl ContractTypeRefSpec for i32 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I32,
        type_def: None,
    };
}

impl ContractTypeRefSpec for u64 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U64,
        type_def: None,
    };
}

impl ContractTypeRefSpec for i64 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I64,
        type_def: None,
    };
}

impl ContractTypeRefSpec for u128 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U128,
        type_def: None,
    };
}

impl ContractTypeRefSpec for i128 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I128,
        type_def: None,
    };
}

impl ContractTypeRefSpec for str {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::String,
        type_def: None,
    };
}

impl ContractTypeRefSpec for [u8] {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Bytes,
        type_def: None,
    };
}

#[cfg(feature = "alloc")]
impl ContractTypeRefSpec for String {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::String,
        type_def: None,
    };
}

#[cfg(feature = "alloc")]
impl<T> ContractTypeRefSpec for Vec<T>
where
    T: ContractTypeRefSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Vec(&type_ref::<T>()),
        type_def: None,
    };
}

impl<T> ContractTypeRefSpec for Option<T>
where
    T: ContractTypeRefSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Option(&type_ref::<T>()),
        type_def: None,
    };
}

impl<T> ContractTypeRefSpec for T
where
    T: ContractTypeSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: T::TYPE_DEF.type_ref(),
        type_def: Some(T::TYPE_DEF),
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
                    type_def: None,
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
);

pub const fn symbol<T: ContractTypeSpec>() -> &'static str {
    T::TYPE_DEF.symbol()
}

pub const fn type_ref<T: ContractTypeRefSpec>()
-> view::ContractTypeView<'static> {
    T::INFO.ty
}

pub const fn maybe_type_def<T: ContractTypeRefSpec>()
-> Option<view::ContractTypeDefView<'static>> {
    T::INFO.type_def
}

pub const fn const_txs_typedef_fragment_stream_len(
    type_defs: &[Option<view::ContractTypeDefView<'_>>],
) -> usize {
    let mut total = 0usize;
    let mut i = 0usize;
    while i < type_defs.len() {
        if let Some(type_def) = type_defs[i] {
            total +=
                view::SpecFragmentView::TypeDef(type_def).const_txs_size();
        }
        i += 1;
    }
    total
}

pub const fn const_txs_typedef_fragment_stream_bytes<const OUT: usize>(
    type_defs: &[Option<view::ContractTypeDefView<'_>>],
) -> [u8; OUT] {
    assert!(OUT == const_txs_typedef_fragment_stream_len(type_defs));
    let mut w = xdrs::txs::ConstWriter::<OUT>::new();
    let mut i = 0usize;
    while i < type_defs.len() {
        if let Some(type_def) = type_defs[i] {
            view::SpecFragmentView::TypeDef(type_def).const_write_txs(&mut w);
        }
        i += 1;
    }
    w.into_bytes()
}
