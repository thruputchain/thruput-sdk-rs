//! Borrowed/const equivalents of the model types.
//!
//! View types use `&'a str` and `&'a [T]` instead of `String` and `Vec<T>`,
//! enabling compile-time spec generation via `const` evaluation.

#[cfg(feature = "owned-model")]
use crate::{
    ContractEnumDef, ContractEnumVariant, ContractEventDef,
    ContractExceptionType, ContractField, ContractSpec, ContractSpecInterface,
    ContractSpecMethod, ContractSpecString, ContractStructDef, ContractType,
    ContractTypeDef, ContractUnionCase, ContractUnionCaseType,
    ContractUnionDef, SpecFragment, SpecInterfaceFragment, SpecMethodFragment,
};
#[cfg(feature = "owned-model")]
use alloc::{boxed::Box, vec::Vec};

use xdrs::*;

#[cfg(feature = "owned-model")]
fn to_spec_string(s: &str) -> ContractSpecString {
    ContractSpecString::try_from(s).expect("spec string conversion")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct ContractSpecView<'a> {
    pub interfaces: &'a [ContractSpecInterfaceView<'a>],
    pub type_defs: &'a [ContractTypeDefView<'a>],
    pub events: &'a [ContractEventDefView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct ContractSpecInterfaceView<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub throws: &'a [ContractExceptionTypeView<'a>],
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
pub struct ContractExceptionTypeView<'a> {
    pub selector: u64,
    pub ty: ContractTypeView<'a>,
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

/// Describes an event emitted via the `LogEvent` syscall. Mirrors the owned
/// `ContractEventDef`: `signature` is topic 0, `topics` are the indexed
/// fields (topics 1..), and `fields` form the XDR-encoded payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct ContractEventDefView<'a> {
    pub name: &'a str,
    pub signature: [u8; 32],
    pub topics: &'a [ContractFieldView<'a>],
    pub fields: &'a [ContractFieldView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
#[repr(i32)]
pub enum SpecFragmentView<'a> {
    Interface(SpecInterfaceFragmentView<'a>) = 0,
    Method(SpecMethodFragmentView<'a>) = 1,
    TypeDef(ContractTypeDefView<'a>) = 2,
    ExportedTypeDef(ContractTypeDefView<'a>) = 3,
    Event(ContractEventDefView<'a>) = 4,
    ExportedEvent(ContractEventDefView<'a>) = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct SpecInterfaceFragmentView<'a> {
    pub interface_name: &'a str,
    pub interface_version: &'a str,
    pub throws: &'a [ContractExceptionTypeView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct SpecMethodFragmentView<'a> {
    pub interface_name: &'a str,
    pub interface_version: &'a str,
    pub method: ContractSpecMethodView<'a>,
}

#[cfg(feature = "owned-model")]
impl From<ContractSpecView<'_>> for ContractSpec {
    fn from(value: ContractSpecView<'_>) -> Self {
        Self {
            interfaces: value
                .interfaces
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            type_defs: value
                .type_defs
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            events: value
                .events
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<ContractEventDefView<'_>> for ContractEventDef {
    fn from(value: ContractEventDefView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            signature: value.signature.into(),
            topics: value
                .topics
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            fields: value
                .fields
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<ContractExceptionTypeView<'_>> for ContractExceptionType {
    fn from(value: ContractExceptionTypeView<'_>) -> Self {
        Self {
            selector: value.selector,
            ty: value.ty.into(),
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<ContractSpecInterfaceView<'_>> for ContractSpecInterface {
    fn from(value: ContractSpecInterfaceView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            vers: to_spec_string(value.version),
            throws: value
                .throws
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            methods: value
                .methods
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<ContractSpecMethodView<'_>> for ContractSpecMethod {
    fn from(value: ContractSpecMethodView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            args: value
                .args
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            return_type: value.return_type.into(),
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<ContractFieldView<'_>> for ContractField {
    fn from(value: ContractFieldView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            ty: value.ty.into(),
        }
    }
}

#[cfg(feature = "owned-model")]
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
            ContractTypeView::Option(inner) => ContractType::Option {
                option_inner: Box::new((*inner).into()),
            },
            ContractTypeView::Vector(inner) => ContractType::Vector {
                vector: inner.into(),
            },
            ContractTypeView::Tuple(items) => ContractType::Tuple {
                tuple: items
                    .iter()
                    .copied()
                    .map(Into::into)
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            },
            ContractTypeView::Named(name) => ContractType::Named {
                named: to_spec_string(name),
            },
            ContractTypeView::U8 => ContractType::U8,
            ContractTypeView::Array(array) => ContractType::Array {
                array: array.into(),
            },
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<ContractArrayTypeView<'_>> for crate::ContractArrayType {
    fn from(value: ContractArrayTypeView<'_>) -> Self {
        Self {
            element: Box::new((*value.element).into()),
            len: value.len,
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<ContractVectorTypeView<'_>> for crate::ContractVectorType {
    fn from(value: ContractVectorTypeView<'_>) -> Self {
        Self {
            element: Box::new((*value.element).into()),
            max_len: value.max_len,
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<ContractTypeDefView<'_>> for ContractTypeDef {
    fn from(value: ContractTypeDefView<'_>) -> Self {
        match value {
            ContractTypeDefView::Struct(def) => {
                ContractTypeDef::TypeDefStruct {
                    struct_def: def.into(),
                }
            }
            ContractTypeDefView::Enum(def) => ContractTypeDef::TypeDefEnum {
                enum_def: def.into(),
            },
            ContractTypeDefView::Union(def) => ContractTypeDef::TypeDefUnion {
                union_def: def.into(),
            },
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<ContractStructDefView<'_>> for ContractStructDef {
    fn from(value: ContractStructDefView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            fields: value
                .fields
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<ContractEnumDefView<'_>> for ContractEnumDef {
    fn from(value: ContractEnumDefView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            variants: value
                .variants
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<ContractEnumVariantView<'_>> for ContractEnumVariant {
    fn from(value: ContractEnumVariantView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            value: value.value,
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<ContractUnionDefView<'_>> for ContractUnionDef {
    fn from(value: ContractUnionDefView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            cases: value
                .cases
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<ContractUnionCaseView<'_>> for ContractUnionCase {
    fn from(value: ContractUnionCaseView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            discriminant: value.discriminant,
            case_type: value.case_type.into(),
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<ContractUnionCaseTypeView<'_>> for ContractUnionCaseType {
    fn from(value: ContractUnionCaseTypeView<'_>) -> Self {
        match value {
            ContractUnionCaseTypeView::Void => {
                ContractUnionCaseType::UnionCaseVoid
            }
            ContractUnionCaseTypeView::Tuple(types) => {
                ContractUnionCaseType::UnionCaseTuple {
                    tuple: types
                        .iter()
                        .copied()
                        .map(Into::into)
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap(),
                }
            }
            ContractUnionCaseTypeView::Struct(fields) => {
                ContractUnionCaseType::UnionCaseStruct {
                    struct_fields: fields
                        .iter()
                        .copied()
                        .map(Into::into)
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap(),
                }
            }
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<SpecFragmentView<'_>> for SpecFragment {
    fn from(value: SpecFragmentView<'_>) -> Self {
        match value {
            SpecFragmentView::Interface(fragment) => {
                SpecFragment::SpecInterface {
                    interface_fragment: fragment.into(),
                }
            }
            SpecFragmentView::Method(fragment) => SpecFragment::SpecMethod {
                method_fragment: fragment.into(),
            },
            SpecFragmentView::TypeDef(def) => SpecFragment::SpecTypeDef {
                type_def: def.into(),
            },
            SpecFragmentView::ExportedTypeDef(def) => {
                SpecFragment::ExportedSpecTypeDef {
                    type_def: def.into(),
                }
            }
            SpecFragmentView::Event(def) => SpecFragment::SpecEvent {
                event_def: def.into(),
            },
            SpecFragmentView::ExportedEvent(def) => {
                SpecFragment::ExportedSpecEvent {
                    event_def: def.into(),
                }
            }
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<SpecInterfaceFragmentView<'_>> for SpecInterfaceFragment {
    fn from(value: SpecInterfaceFragmentView<'_>) -> Self {
        Self {
            interface_name: to_spec_string(value.interface_name),
            interface_version: to_spec_string(value.interface_version),
            throws: value
                .throws
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        }
    }
}

#[cfg(feature = "owned-model")]
impl From<SpecMethodFragmentView<'_>> for SpecMethodFragment {
    fn from(value: SpecMethodFragmentView<'_>) -> Self {
        Self {
            interface_name: to_spec_string(value.interface_name),
            interface_version: to_spec_string(value.interface_version),
            method: value.method.into(),
        }
    }
}
