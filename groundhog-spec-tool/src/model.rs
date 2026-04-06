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
}

impl RawInterfaceFragment {
    pub fn free_fn(
        package_name: impl Into<String>,
        interface_name: impl Into<String>,
        interface_version: impl Into<String>,
    ) -> Self {
        Self {
            package_name: package_name.into(),
            interface_name: interface_name.into(),
            interface_version: interface_version.into(),
        }
    }

    pub fn impl_backed(
        package_name: impl Into<String>,
        interface_name: impl Into<String>,
        interface_version: impl Into<String>,
    ) -> Self {
        Self {
            package_name: package_name.into(),
            interface_name: interface_name.into(),
            interface_version: interface_version.into(),
        }
    }
}
