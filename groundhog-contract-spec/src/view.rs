#[cfg(feature = "alloc")]
use crate::model::{
    ContractEnumDef, ContractEnumVariant, ContractField, ContractSpec,
    ContractSpecInterface, ContractSpecMethod, ContractStructDef,
    ContractType, ContractTypeDef, ContractUnionCase, ContractUnionDef,
    SpecFragment, SpecInterfaceFragment, SpecMethodFragment,
};
#[cfg(feature = "alloc")]
use alloc::{borrow::ToOwned, boxed::Box};

use xdrs::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct ContractSpecView<'a> {
    pub interfaces: &'a [ContractSpecInterfaceView<'a>],
    pub type_defs: &'a [ContractTypeDefView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct ContractSpecInterfaceView<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub throws: &'a [ContractTypeView<'a>],
    pub methods: &'a [ContractSpecMethodView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct ContractSpecMethodView<'a> {
    pub name: &'a str,
    pub args: &'a [ContractFieldView<'a>],
    pub return_type: ContractTypeView<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct ContractFieldView<'a> {
    pub name: &'a str,
    pub ty: ContractTypeView<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
#[repr(i32)]
pub enum ContractTypeView<'a> {
    Void = 0,
    Bool = 1,
    U32 = 2,
    I32 = 3,
    U64 = 4,
    I64 = 5,
    U128 = 6,
    I128 = 7,
    String = 8,
    Bytes = 9,
    Address = 10,
    Option(&'a ContractTypeView<'a>) = 11,
    Vector(ContractVectorTypeView<'a>) = 12,
    Tuple(&'a [ContractTypeView<'a>]) = 13,
    Named(&'a str) = 14,
    U8 = 15,
    Array(ContractArrayTypeView<'a>) = 16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct ContractArrayTypeView<'a> {
    pub element: &'a ContractTypeView<'a>,
    pub len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct ContractVectorTypeView<'a> {
    pub element: &'a ContractTypeView<'a>,
    pub max_len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
#[repr(i32)]
pub enum ContractTypeDefView<'a> {
    Struct(ContractStructDefView<'a>) = 0,
    Enum(ContractEnumDefView<'a>) = 1,
    Union(ContractUnionDefView<'a>) = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractTypeDefsView<'a> {
    Empty,
    One(ContractTypeDefView<'a>),
    Many(&'a [ContractTypeDefsView<'a>]),
}

impl<'a> ContractTypeDefView<'a> {
    pub const fn symbol(self) -> &'a str {
        match self {
            Self::Struct(def) => def.name,
            Self::Enum(def) => def.name,
            Self::Union(def) => def.name,
        }
    }

    pub const fn type_ref(self) -> ContractTypeView<'a> {
        ContractTypeView::Named(self.symbol())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct ContractStructDefView<'a> {
    pub name: &'a str,
    pub fields: &'a [ContractFieldView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct ContractEnumDefView<'a> {
    pub name: &'a str,
    pub variants: &'a [ContractEnumVariantView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct ContractEnumVariantView<'a> {
    pub name: &'a str,
    pub value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct ContractUnionDefView<'a> {
    pub name: &'a str,
    pub cases: &'a [ContractUnionCaseView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct ContractUnionCaseView<'a> {
    pub name: &'a str,
    pub discriminant: i32,
    pub case_type: ContractUnionCaseTypeView<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
#[repr(i32)]
pub enum ContractUnionCaseTypeView<'a> {
    Void = 0,
    Tuple(&'a [ContractTypeView<'a>]) = 1,
    Struct(&'a [ContractFieldView<'a>]) = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
#[repr(i32)]
pub enum SpecFragmentView<'a> {
    Interface(SpecInterfaceFragmentView<'a>) = 0,
    Method(SpecMethodFragmentView<'a>) = 1,
    TypeDef(ContractTypeDefView<'a>) = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct SpecInterfaceFragmentView<'a> {
    pub interface_name: &'a str,
    pub interface_version: &'a str,
    pub throws: &'a [ContractTypeView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct SpecMethodFragmentView<'a> {
    pub interface_name: &'a str,
    pub interface_version: &'a str,
    pub method: ContractSpecMethodView<'a>,
}

#[cfg(feature = "alloc")]
impl From<ContractSpecView<'_>> for ContractSpec {
    fn from(value: ContractSpecView<'_>) -> Self {
        Self {
            interfaces: value
                .interfaces
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
            type_defs: value
                .type_defs
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractSpecInterfaceView<'_>> for ContractSpecInterface {
    fn from(value: ContractSpecInterfaceView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            version: value.version.to_owned(),
            throws: value.throws.iter().copied().map(Into::into).collect(),
            methods: value.methods.iter().copied().map(Into::into).collect(),
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractSpecMethodView<'_>> for ContractSpecMethod {
    fn from(value: ContractSpecMethodView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            args: value.args.iter().copied().map(Into::into).collect(),
            return_type: value.return_type.into(),
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractFieldView<'_>> for ContractField {
    fn from(value: ContractFieldView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            ty: value.ty.into(),
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractTypeView<'_>> for ContractType {
    fn from(value: ContractTypeView<'_>) -> Self {
        match value {
            ContractTypeView::Void => ContractType::Void,
            ContractTypeView::Bool => ContractType::Bool,
            ContractTypeView::U32 => ContractType::U32,
            ContractTypeView::I32 => ContractType::I32,
            ContractTypeView::U64 => ContractType::U64,
            ContractTypeView::I64 => ContractType::I64,
            ContractTypeView::U128 => ContractType::U128,
            ContractTypeView::I128 => ContractType::I128,
            ContractTypeView::String => ContractType::String,
            ContractTypeView::Bytes => ContractType::Bytes,
            ContractTypeView::Address => ContractType::Address,
            ContractTypeView::Option(inner) => {
                ContractType::Option(Box::new((*inner).into()))
            }
            ContractTypeView::Vector(inner) => {
                ContractType::Vector(inner.into())
            }
            ContractTypeView::Tuple(items) => ContractType::Tuple(
                items.iter().copied().map(Into::into).collect(),
            ),
            ContractTypeView::Named(name) => {
                ContractType::Named(name.to_owned())
            }
            ContractTypeView::U8 => ContractType::U8,
            ContractTypeView::Array(array) => {
                ContractType::Array(array.into())
            }
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractArrayTypeView<'_>> for crate::model::ContractArrayType {
    fn from(value: ContractArrayTypeView<'_>) -> Self {
        Self {
            element: Box::new((*value.element).into()),
            len: value.len,
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractVectorTypeView<'_>> for crate::model::ContractVectorType {
    fn from(value: ContractVectorTypeView<'_>) -> Self {
        Self {
            element: Box::new((*value.element).into()),
            max_len: value.max_len,
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractTypeDefView<'_>> for ContractTypeDef {
    fn from(value: ContractTypeDefView<'_>) -> Self {
        match value {
            ContractTypeDefView::Struct(def) => {
                ContractTypeDef::Struct(def.into())
            }
            ContractTypeDefView::Enum(def) => {
                ContractTypeDef::Enum(def.into())
            }
            ContractTypeDefView::Union(def) => {
                ContractTypeDef::Union(def.into())
            }
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractStructDefView<'_>> for ContractStructDef {
    fn from(value: ContractStructDefView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            fields: value.fields.iter().copied().map(Into::into).collect(),
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractEnumDefView<'_>> for ContractEnumDef {
    fn from(value: ContractEnumDefView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            variants: value.variants.iter().copied().map(Into::into).collect(),
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractEnumVariantView<'_>> for ContractEnumVariant {
    fn from(value: ContractEnumVariantView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            value: value.value,
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractUnionDefView<'_>> for ContractUnionDef {
    fn from(value: ContractUnionDefView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            cases: value.cases.iter().copied().map(Into::into).collect(),
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractUnionCaseView<'_>> for ContractUnionCase {
    fn from(value: ContractUnionCaseView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            discriminant: value.discriminant,
            case_type: value.case_type.into(),
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractUnionCaseTypeView<'_>>
    for crate::model::ContractUnionCaseType
{
    fn from(value: ContractUnionCaseTypeView<'_>) -> Self {
        match value {
            ContractUnionCaseTypeView::Void => {
                crate::model::ContractUnionCaseType::Void
            }
            ContractUnionCaseTypeView::Tuple(types) => {
                crate::model::ContractUnionCaseType::Tuple(
                    types.iter().copied().map(Into::into).collect(),
                )
            }
            ContractUnionCaseTypeView::Struct(fields) => {
                crate::model::ContractUnionCaseType::Struct(
                    fields.iter().copied().map(Into::into).collect(),
                )
            }
        }
    }
}

#[cfg(feature = "alloc")]
impl From<SpecFragmentView<'_>> for SpecFragment {
    fn from(value: SpecFragmentView<'_>) -> Self {
        match value {
            SpecFragmentView::Interface(fragment) => {
                SpecFragment::Interface(fragment.into())
            }
            SpecFragmentView::Method(fragment) => {
                SpecFragment::Method(fragment.into())
            }
            SpecFragmentView::TypeDef(def) => {
                SpecFragment::TypeDef(def.into())
            }
        }
    }
}

#[cfg(feature = "alloc")]
impl From<SpecInterfaceFragmentView<'_>> for SpecInterfaceFragment {
    fn from(value: SpecInterfaceFragmentView<'_>) -> Self {
        Self {
            interface_name: value.interface_name.to_owned(),
            interface_version: value.interface_version.to_owned(),
            throws: value.throws.iter().copied().map(Into::into).collect(),
        }
    }
}

#[cfg(feature = "alloc")]
impl From<SpecMethodFragmentView<'_>> for SpecMethodFragment {
    fn from(value: SpecMethodFragmentView<'_>) -> Self {
        Self {
            interface_name: value.interface_name.to_owned(),
            interface_version: value.interface_version.to_owned(),
            method: value.method.into(),
        }
    }
}
