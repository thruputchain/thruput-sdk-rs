pub use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumVariant, ContractField, ContractSpec,
    ContractSpecInterface, ContractSpecMethod, ContractStructDef,
    ContractType, ContractTypeDef, ContractUnionCase, ContractUnionDef,
    SpecFragment, SpecInterfaceFragment, SpecMethodFragment,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InterfaceId {
    pub interface_name: String,
    pub interface_version: String,
}

impl InterfaceId {
    pub fn new(
        interface_name: impl Into<String>,
        interface_version: impl Into<String>,
    ) -> Self {
        Self {
            interface_name: interface_name.into(),
            interface_version: interface_version.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawInterfaceFragment {
    pub package_name: String,
    pub interface_name: String,
    pub interface_version: String,
    pub throws: Vec<ContractType>,
}

impl RawInterfaceFragment {
    pub fn new(
        package_name: impl Into<String>,
        interface_name: impl Into<String>,
        interface_version: impl Into<String>,
        throws: Vec<ContractType>,
    ) -> Self {
        Self {
            package_name: package_name.into(),
            interface_name: interface_name.into(),
            interface_version: interface_version.into(),
            throws,
        }
    }

    pub fn impl_backed(
        package_name: impl Into<String>,
        interface_name: impl Into<String>,
        interface_version: impl Into<String>,
    ) -> Self {
        Self::new(package_name, interface_name, interface_version, Vec::new())
    }
}

impl From<RawInterfaceFragment> for SpecInterfaceFragment {
    fn from(value: RawInterfaceFragment) -> Self {
        Self {
            package_name: value.package_name,
            interface_name: value.interface_name,
            interface_version: value.interface_version,
            throws: value.throws,
        }
    }
}

impl From<SpecInterfaceFragment> for RawInterfaceFragment {
    fn from(value: SpecInterfaceFragment) -> Self {
        Self {
            package_name: value.package_name,
            interface_name: value.interface_name,
            interface_version: value.interface_version,
            throws: value.throws,
        }
    }
}
