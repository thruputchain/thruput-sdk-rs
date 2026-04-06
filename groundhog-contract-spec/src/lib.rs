#![no_std]

extern crate alloc;

pub mod model;
pub mod view;

pub use model::*;
pub use view::*;

pub trait ContractTypeSpec {
    const TYPE_DEF: view::ContractTypeDefView<'static>;
}

pub fn symbol<T: ContractTypeSpec>() -> &'static str {
    T::TYPE_DEF.symbol()
}

pub fn type_ref<T: ContractTypeSpec>() -> view::ContractTypeView<'static> {
    T::TYPE_DEF.type_ref()
}
