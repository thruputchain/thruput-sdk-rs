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

pub trait ContractTypeRefSpec {
    const TYPE_REF: view::ContractTypeView<'static>;
}

impl ContractTypeRefSpec for () {
    const TYPE_REF: view::ContractTypeView<'static> =
        view::ContractTypeView::Void;
}

impl ContractTypeRefSpec for bool {
    const TYPE_REF: view::ContractTypeView<'static> =
        view::ContractTypeView::Bool;
}

impl ContractTypeRefSpec for u32 {
    const TYPE_REF: view::ContractTypeView<'static> =
        view::ContractTypeView::U32;
}

impl ContractTypeRefSpec for i32 {
    const TYPE_REF: view::ContractTypeView<'static> =
        view::ContractTypeView::I32;
}

impl ContractTypeRefSpec for u64 {
    const TYPE_REF: view::ContractTypeView<'static> =
        view::ContractTypeView::U64;
}

impl ContractTypeRefSpec for i64 {
    const TYPE_REF: view::ContractTypeView<'static> =
        view::ContractTypeView::I64;
}

impl ContractTypeRefSpec for u128 {
    const TYPE_REF: view::ContractTypeView<'static> =
        view::ContractTypeView::U128;
}

impl ContractTypeRefSpec for i128 {
    const TYPE_REF: view::ContractTypeView<'static> =
        view::ContractTypeView::I128;
}

impl ContractTypeRefSpec for str {
    const TYPE_REF: view::ContractTypeView<'static> =
        view::ContractTypeView::String;
}

impl ContractTypeRefSpec for [u8] {
    const TYPE_REF: view::ContractTypeView<'static> =
        view::ContractTypeView::Bytes;
}

#[cfg(feature = "alloc")]
impl ContractTypeRefSpec for String {
    const TYPE_REF: view::ContractTypeView<'static> =
        view::ContractTypeView::String;
}

#[cfg(feature = "alloc")]
impl<T> ContractTypeRefSpec for Vec<T>
where
    T: ContractTypeRefSpec,
{
    const TYPE_REF: view::ContractTypeView<'static> =
        view::ContractTypeView::Vec(&T::TYPE_REF);
}

impl<T> ContractTypeRefSpec for Option<T>
where
    T: ContractTypeRefSpec,
{
    const TYPE_REF: view::ContractTypeView<'static> =
        view::ContractTypeView::Option(&T::TYPE_REF);
}

impl<T> ContractTypeRefSpec for T
where
    T: ContractTypeSpec,
{
    const TYPE_REF: view::ContractTypeView<'static> = T::TYPE_DEF.type_ref();
}

macro_rules! impl_tuple_type_ref_spec {
    ($(($($name:ident),+)),+ $(,)?) => {
        $(
            impl<$($name),+> ContractTypeRefSpec for ($($name,)+)
            where
                $($name: ContractTypeRefSpec),+
            {
                const TYPE_REF: view::ContractTypeView<'static> =
                    view::ContractTypeView::Tuple(&[$($name::TYPE_REF),+]);
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
    T::TYPE_REF
}
