#![no_std]

extern crate alloc;

pub mod model;
pub mod view;

pub use model::*;
pub use view::*;

pub trait ContractTypeSpec {
    const SYMBOL: &'static str;
    const TYPE_REF: view::ContractTypeView<'static>;
    const TYPE_DEF: view::ContractTypeDefView<'static>;
}

