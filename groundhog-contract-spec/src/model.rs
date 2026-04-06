use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use xdrs::*;

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractSpec {
    pub interfaces: Vec<ContractSpecInterface>,
    pub type_defs: Vec<ContractTypeDef>,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractSpecInterface {
    pub name: String,
    pub version: String,
    pub throws: Vec<ContractType>,
    pub methods: Vec<ContractSpecMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractSpecMethod {
    pub name: String,
    pub args: Vec<ContractField>,
    pub return_type: ContractType,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractField {
    pub name: String,
    pub ty: ContractType,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum ContractType {
    Void,
    Bool,
    U32,
    I32,
    U64,
    I64,
    String,
    Bytes,
    Address,
    Option(Box<ContractType>),
    Vec(Box<ContractType>),
    Tuple(Vec<ContractType>),
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum ContractTypeDef {
    Struct(ContractStructDef),
    Enum(ContractEnumDef),
    Union(ContractUnionDef),
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractStructDef {
    pub name: String,
    pub fields: Vec<ContractField>,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractEnumDef {
    pub name: String,
    pub variants: Vec<ContractEnumVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractEnumVariant {
    pub name: String,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractUnionDef {
    pub name: String,
    pub cases: Vec<ContractUnionCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractUnionCase {
    pub name: String,
    pub discriminant: i32,
    pub case_type: Option<ContractType>,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum SpecFragment {
    Interface(SpecInterfaceFragment),
    Method(SpecMethodFragment),
    TypeDef(ContractTypeDef),
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct SpecInterfaceFragment {
    pub package_name: String,
    pub interface_name: String,
    pub interface_version: String,
    pub throws: Vec<ContractType>,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct SpecMethodFragment {
    pub package_name: String,
    pub interface_name: String,
    pub interface_version: String,
    pub method: ContractSpecMethod,
}
