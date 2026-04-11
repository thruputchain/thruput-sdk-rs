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
#[cfg(feature = "alloc")]
use xdrs::VecM;

mod groundhog_xdr_types;

pub trait ContractTypeSpec {
    const TYPE_DEF: view::ContractTypeDefView<'static>;
    const TYPE_DEFS: view::ContractTypeDefsView<'static> =
        view::ContractTypeDefsView::One(Self::TYPE_DEF);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractTypeInfo<'a> {
    pub ty: view::ContractTypeView<'a>,
    pub type_defs: view::ContractTypeDefsView<'a>,
}

pub trait ContractTypeRefSpec {
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
            element: &view::ContractTypeView::U8,
            max_len: u32::MAX,
        }),
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl<'a> ContractTypeRefSpec for &'a [u8] {
    const INFO: ContractTypeInfo<'static> =
        <[u8] as ContractTypeRefSpec>::INFO;
}

impl<'a> ContractTypeRefSpec for &'a str {
    const INFO: ContractTypeInfo<'static> = <str as ContractTypeRefSpec>::INFO;
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
            element: &type_ref::<T>(),
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
            element: &type_ref::<T>(),
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
        ty: view::ContractTypeView::Option(&type_ref::<T>()),
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
