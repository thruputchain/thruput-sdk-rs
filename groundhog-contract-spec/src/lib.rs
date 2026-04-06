#![no_std]

extern crate alloc;

pub mod model;
pub mod view;

pub use model::*;
pub use view::*;

pub trait ContractTypeSpec {
    const TYPE_DEF: view::ContractTypeDefView<'static>;
    const SYMBOL: &'static str = Self::TYPE_DEF.symbol();
    const TYPE_REF: view::ContractTypeView<'static> =
        Self::TYPE_DEF.type_ref();
}
