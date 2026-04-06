use alloc::boxed::Box;
use alloc::borrow::ToOwned;
use crate::model::{
    ContractEnumDef, ContractEnumVariant, ContractField, ContractSpec,
    ContractSpecInterface, ContractSpecMethod, ContractStructDef, ContractType,
    ContractTypeDef, ContractUnionCase, ContractUnionDef, SpecFragment,
    SpecInterfaceFragment, SpecMethodFragment,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractSpecView<'a> {
    pub interfaces: &'a [ContractSpecInterfaceView<'a>],
    pub type_defs: &'a [ContractTypeDefView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractSpecInterfaceView<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub throws: &'a [ContractTypeView<'a>],
    pub methods: &'a [ContractSpecMethodView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractSpecMethodView<'a> {
    pub name: &'a str,
    pub args: &'a [ContractFieldView<'a>],
    pub return_type: ContractTypeView<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractFieldView<'a> {
    pub name: &'a str,
    pub ty: ContractTypeView<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractTypeView<'a> {
    Void,
    Bool,
    U32,
    I32,
    U64,
    I64,
    String,
    Bytes,
    Address,
    Option(&'a ContractTypeView<'a>),
    Vec(&'a ContractTypeView<'a>),
    Tuple(&'a [ContractTypeView<'a>]),
    Named(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractTypeDefView<'a> {
    Struct(ContractStructDefView<'a>),
    Enum(ContractEnumDefView<'a>),
    Union(ContractUnionDefView<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractStructDefView<'a> {
    pub name: &'a str,
    pub fields: &'a [ContractFieldView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractEnumDefView<'a> {
    pub name: &'a str,
    pub variants: &'a [ContractEnumVariantView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractEnumVariantView<'a> {
    pub name: &'a str,
    pub value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractUnionDefView<'a> {
    pub name: &'a str,
    pub cases: &'a [ContractUnionCaseView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractUnionCaseView<'a> {
    pub name: &'a str,
    pub discriminant: i32,
    pub case_type: Option<ContractTypeView<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecFragmentView<'a> {
    Interface(SpecInterfaceFragmentView<'a>),
    Method(SpecMethodFragmentView<'a>),
    TypeDef(ContractTypeDefView<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecInterfaceFragmentView<'a> {
    pub package_name: &'a str,
    pub interface_name: &'a str,
    pub interface_version: &'a str,
    pub throws: &'a [ContractTypeView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecMethodFragmentView<'a> {
    pub package_name: &'a str,
    pub interface_name: &'a str,
    pub interface_version: &'a str,
    pub method: ContractSpecMethodView<'a>,
}

impl From<ContractSpecView<'_>> for ContractSpec {
    fn from(value: ContractSpecView<'_>) -> Self {
        Self {
            interfaces: value.interfaces.iter().copied().map(Into::into).collect(),
            type_defs: value.type_defs.iter().copied().map(Into::into).collect(),
        }
    }
}

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

impl From<ContractSpecMethodView<'_>> for ContractSpecMethod {
    fn from(value: ContractSpecMethodView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            args: value.args.iter().copied().map(Into::into).collect(),
            return_type: value.return_type.into(),
        }
    }
}

impl From<ContractFieldView<'_>> for ContractField {
    fn from(value: ContractFieldView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            ty: value.ty.into(),
        }
    }
}

impl From<ContractTypeView<'_>> for ContractType {
    fn from(value: ContractTypeView<'_>) -> Self {
        match value {
            ContractTypeView::Void => ContractType::Void,
            ContractTypeView::Bool => ContractType::Bool,
            ContractTypeView::U32 => ContractType::U32,
            ContractTypeView::I32 => ContractType::I32,
            ContractTypeView::U64 => ContractType::U64,
            ContractTypeView::I64 => ContractType::I64,
            ContractTypeView::String => ContractType::String,
            ContractTypeView::Bytes => ContractType::Bytes,
            ContractTypeView::Address => ContractType::Address,
            ContractTypeView::Option(inner) => {
                ContractType::Option(Box::new((*inner).into()))
            }
            ContractTypeView::Vec(inner) => {
                ContractType::Vec(Box::new((*inner).into()))
            }
            ContractTypeView::Tuple(items) => {
                ContractType::Tuple(items.iter().copied().map(Into::into).collect())
            }
            ContractTypeView::Named(name) => ContractType::Named(name.to_owned()),
        }
    }
}

impl From<ContractTypeDefView<'_>> for ContractTypeDef {
    fn from(value: ContractTypeDefView<'_>) -> Self {
        match value {
            ContractTypeDefView::Struct(def) => ContractTypeDef::Struct(def.into()),
            ContractTypeDefView::Enum(def) => ContractTypeDef::Enum(def.into()),
            ContractTypeDefView::Union(def) => ContractTypeDef::Union(def.into()),
        }
    }
}

impl From<ContractStructDefView<'_>> for ContractStructDef {
    fn from(value: ContractStructDefView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            fields: value.fields.iter().copied().map(Into::into).collect(),
        }
    }
}

impl From<ContractEnumDefView<'_>> for ContractEnumDef {
    fn from(value: ContractEnumDefView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            variants: value.variants.iter().copied().map(Into::into).collect(),
        }
    }
}

impl From<ContractEnumVariantView<'_>> for ContractEnumVariant {
    fn from(value: ContractEnumVariantView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            value: value.value,
        }
    }
}

impl From<ContractUnionDefView<'_>> for ContractUnionDef {
    fn from(value: ContractUnionDefView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            cases: value.cases.iter().copied().map(Into::into).collect(),
        }
    }
}

impl From<ContractUnionCaseView<'_>> for ContractUnionCase {
    fn from(value: ContractUnionCaseView<'_>) -> Self {
        Self {
            name: value.name.to_owned(),
            discriminant: value.discriminant,
            case_type: value.case_type.map(Into::into),
        }
    }
}

impl From<SpecFragmentView<'_>> for SpecFragment {
    fn from(value: SpecFragmentView<'_>) -> Self {
        match value {
            SpecFragmentView::Interface(fragment) => {
                SpecFragment::Interface(fragment.into())
            }
            SpecFragmentView::Method(fragment) => SpecFragment::Method(fragment.into()),
            SpecFragmentView::TypeDef(def) => SpecFragment::TypeDef(def.into()),
        }
    }
}

impl From<SpecInterfaceFragmentView<'_>> for SpecInterfaceFragment {
    fn from(value: SpecInterfaceFragmentView<'_>) -> Self {
        Self {
            package_name: value.package_name.to_owned(),
            interface_name: value.interface_name.to_owned(),
            interface_version: value.interface_version.to_owned(),
            throws: value.throws.iter().copied().map(Into::into).collect(),
        }
    }
}

impl From<SpecMethodFragmentView<'_>> for SpecMethodFragment {
    fn from(value: SpecMethodFragmentView<'_>) -> Self {
        Self {
            package_name: value.package_name.to_owned(),
            interface_name: value.interface_name.to_owned(),
            interface_version: value.interface_version.to_owned(),
            method: value.method.into(),
        }
    }
}
