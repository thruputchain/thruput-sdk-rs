use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use xdrs::*;

/// A contract's complete spec: all interfaces and their type definitions.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractSpec {
    pub interfaces: Vec<ContractSpecInterface>,
    pub type_defs: Vec<ContractTypeDef>,
}

/// A single interface exposed by a contract, with version, methods, and error types.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractSpecInterface {
    pub name: String,
    pub version: String,
    pub throws: Vec<ContractType>,
    pub methods: Vec<ContractSpecMethod>,
}

/// A method in a contract interface, with typed arguments and return type.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractSpecMethod {
    pub name: String,
    pub args: Vec<ContractField>,
    pub return_type: ContractType,
}

/// A named, typed field used in method arguments and struct definitions.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractField {
    pub name: String,
    pub ty: ContractType,
}

/// A type reference in a contract spec (primitives, collections, or named user types).
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
    Vector(ContractVectorType) = 12,
    Tuple(Vec<ContractType>) = 13,
    Named(String) = 14,
    U8 = 15,
    Array(ContractArrayType) = 16,
}

/// A fixed-length array type: element type and length.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractArrayType {
    pub element: Box<ContractType>,
    pub len: u32,
}

/// A variable-length vector type: element type and maximum length.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractVectorType {
    pub element: Box<ContractType>,
    pub max_len: u32,
}

/// A user-defined type definition: struct, enum, or union.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum ContractTypeDef {
    Struct(ContractStructDef) = 0,
    Enum(ContractEnumDef) = 1,
    Union(ContractUnionDef) = 2,
}

/// Definition of a struct type: name and ordered fields.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractStructDef {
    pub name: String,
    pub fields: Vec<ContractField>,
}

/// Definition of an enum type: name and discriminant variants.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractEnumDef {
    pub name: String,
    pub variants: Vec<ContractEnumVariant>,
}

/// A single enum variant with its name and integer discriminant.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractEnumVariant {
    pub name: String,
    pub value: i32,
}

/// Definition of a union (tagged enum) type: name and cases.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractUnionDef {
    pub name: String,
    pub cases: Vec<ContractUnionCase>,
}

/// A single union case: name, discriminant, and payload type.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractUnionCase {
    pub name: String,
    pub discriminant: i32,
    pub case_type: ContractUnionCaseType,
}

/// The payload type for a union case: no data, positional fields, or named fields.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum ContractUnionCaseType {
    Void = 0,
    Tuple(Vec<ContractType>) = 1,
    Struct(Vec<ContractField>) = 2,
}

/// A raw spec fragment emitted during compilation, before canonicalization.
///
/// Fragments are assembled into a [`ContractSpec`] by the spec tool.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum SpecFragment {
    /// Declares an interface with its name, version, and error types.
    Interface(SpecInterfaceFragment) = 0,
    /// Declares a method belonging to an interface.
    Method(SpecMethodFragment) = 1,
    /// Declares a user-defined type needed by the spec.
    TypeDef(ContractTypeDef) = 2,
}

/// Fragment declaring an interface's identity and error types.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct SpecInterfaceFragment {
    pub interface_name: String,
    pub interface_version: String,
    pub throws: Vec<ContractType>,
}

/// Fragment declaring a method within an interface.
#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct SpecMethodFragment {
    pub interface_name: String,
    pub interface_version: String,
    pub method: ContractSpecMethod,
}
