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
    Option(Box<ContractType>) = 11,
    Vec(Box<ContractType>) = 12,
    Tuple(Vec<ContractType>) = 13,
    Named(String) = 14,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum ContractTypeDef {
    Struct(ContractStructDef) = 0,
    Enum(ContractEnumDef) = 1,
    Union(ContractUnionDef) = 2,
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
    Interface(SpecInterfaceFragment) = 0,
    Method(SpecMethodFragment) = 1,
    TypeDef(ContractTypeDef) = 2,
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
