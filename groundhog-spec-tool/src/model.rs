#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InterfaceId {
    pub interface_name: String,
    pub interface_version: String,
}

impl InterfaceId {
    pub fn new(interface_name: impl Into<String>, interface_version: impl Into<String>) -> Self {
        Self {
            interface_name: interface_name.into(),
            interface_version: interface_version.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawInterfaceFragment {
    pub package_name: String,
    pub self_type_key: Option<String>,
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
            self_type_key: None,
            interface_name: interface_name.into(),
            interface_version: interface_version.into(),
        }
    }

    pub fn impl_backed(
        package_name: impl Into<String>,
        self_type_key: impl Into<String>,
        interface_name: impl Into<String>,
        interface_version: impl Into<String>,
    ) -> Self {
        Self {
            package_name: package_name.into(),
            self_type_key: Some(self_type_key.into()),
            interface_name: interface_name.into(),
            interface_version: interface_version.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecToolError {
    NoInterfaceFragments,
    MultipleImplTypes(String, String),
    InvalidRawFragment(String),
    Wasm(String),
}

impl core::fmt::Display for SpecToolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoInterfaceFragments => write!(f, "no interface fragments"),
            Self::MultipleImplTypes(left, right) => {
                write!(f, "multiple impl-backed self types: {left}, {right}")
            }
            Self::InvalidRawFragment(msg) | Self::Wasm(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for SpecToolError {}
