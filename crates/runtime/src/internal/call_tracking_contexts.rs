//! Call-tracking data for use in debug_context

use core::fmt::{self, Display};

pub struct CallMethod<'a> {
    iface: Option<&'a str>,
    iface_vers: &'a str,
    name: &'a str,
}

impl<'a> CallMethod<'a> {
    pub fn new(
        iface: Option<&'a str>,
        iface_vers: &'a str,
        name: &'a str,
    ) -> Self {
        Self {
            iface,
            iface_vers,
            name,
        }
    }
}

impl Display for CallMethod<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Calling  {}[v{}]::{}(...)",
            self.iface.unwrap_or("_unnamed_interface"),
            self.iface_vers,
            self.name
        )
    }
}

pub struct HandlingMethod<'a> {
    iface: Option<&'a str>,
    iface_vers: &'a str,
    name: &'a str,
}

impl<'a> HandlingMethod<'a> {
    pub fn new(
        iface: Option<&'a str>,
        iface_vers: &'a str,
        name: &'a str,
    ) -> Self {
        Self {
            iface,
            iface_vers,
            name,
        }
    }
}

impl Display for HandlingMethod<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Handling {}[v{}]::{}(...)",
            self.iface.unwrap_or("_unnamed_interface"),
            self.iface_vers,
            self.name
        )
    }
}
