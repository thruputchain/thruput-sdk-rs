#![no_std]

use groundhog_xdr::contract::ContractEntrypointID;
use groundhog_xdr::types::{
    BlockHeight, ContractAddress, Ed25519PublicKey, Ed25519Signature,
    SHA256Hash,
};

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
    const TYPE_DEFS: view::ContractTypeDefsView<'static> =
        view::ContractTypeDefsView::One(Self::TYPE_DEF);
    const REQUIRES_ALLOC: bool = false;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractTypeInfo<'a> {
    pub ty: view::ContractTypeView<'a>,
    pub type_defs: view::ContractTypeDefsView<'a>,
}

pub trait ContractTypeRefSpec {
    const INFO: ContractTypeInfo<'static>;
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for () {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Void,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for bool {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Bool,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for u8 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U8,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for u32 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U32,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for i32 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I32,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for u64 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U64,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for i64 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I64,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for u128 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U128,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for i128 {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I128,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for str {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::String,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for [u8] {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Vec(&view::ContractTypeView::U8),
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl<'a> ContractTypeRefSpec for &'a [u8] {
    const INFO: ContractTypeInfo<'static> =
        <[u8] as ContractTypeRefSpec>::INFO;
    const REQUIRES_ALLOC: bool = false;
}

impl<'a> ContractTypeRefSpec for &'a str {
    const INFO: ContractTypeInfo<'static> = <str as ContractTypeRefSpec>::INFO;
    const REQUIRES_ALLOC: bool = false;
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
        type_defs: type_defs::<T>(),
    };
    const REQUIRES_ALLOC: bool = T::REQUIRES_ALLOC;
}

impl ContractTypeRefSpec for ContractAddress {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Address,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for SHA256Hash {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Bytes,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for Ed25519Signature {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Bytes,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for Ed25519PublicKey {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Bytes,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for ContractEntrypointID {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Bytes,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

impl ContractTypeRefSpec for BlockHeight {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I64,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = false;
}

#[cfg(feature = "alloc")]
impl ContractTypeRefSpec for String {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::String,
        type_defs: view::ContractTypeDefsView::Empty,
    };
    const REQUIRES_ALLOC: bool = true;
}

#[cfg(feature = "alloc")]
impl<T> ContractTypeRefSpec for Vec<T>
where
    T: ContractTypeRefSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Vec(&type_ref::<T>()),
        type_defs: type_defs::<T>(),
    };
    const REQUIRES_ALLOC: bool = true;
}

impl<T> ContractTypeRefSpec for Option<T>
where
    T: ContractTypeRefSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Option(&type_ref::<T>()),
        type_defs: type_defs::<T>(),
    };
    const REQUIRES_ALLOC: bool = T::REQUIRES_ALLOC;
}

impl<T> ContractTypeRefSpec for T
where
    T: ContractTypeSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: T::TYPE_DEF.type_ref(),
        type_defs: T::TYPE_DEFS,
    };
    const REQUIRES_ALLOC: bool = T::REQUIRES_ALLOC;
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
                const REQUIRES_ALLOC: bool = false $(|| $name::REQUIRES_ALLOC)+;
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

pub const fn type_ref<T: ContractTypeRefSpec + ?Sized>()
-> view::ContractTypeView<'static> {
    T::INFO.ty
}

pub const fn type_defs<T: ContractTypeRefSpec + ?Sized>()
-> view::ContractTypeDefsView<'static> {
    T::INFO.type_defs
}

pub const fn requires_alloc<T: ContractTypeRefSpec + ?Sized>() -> bool {
    T::REQUIRES_ALLOC
}

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

pub const fn const_txs_typedef_fragment_closure_bytes<const OUT: usize>(
    type_defs: view::ContractTypeDefsView<'_>,
) -> [u8; OUT] {
    assert!(OUT == const_txs_typedef_fragment_closure_len(type_defs));
    let mut w = xdrs::txs::ConstWriter::<OUT>::new();
    const_write_txs_typedef_fragment_closure(&mut w, type_defs);
    w.into_bytes()
}
