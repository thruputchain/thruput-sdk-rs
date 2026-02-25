#[cfg(feature = "call_tracking")]
#[macro_export]
macro_rules! with_call_tracking {
    { $call_info:expr, $body:expr } => {
        ::sdk::with_debug_context(&$call_info, || { $body })
    };
}

#[cfg(not(feature = "call_tracking"))]
#[macro_export]
macro_rules! with_call_tracking {
    { $call_info:expr, $body:expr } => {
        $body
    };
}

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
            "Call   {}[v{}]::{}(...)",
            self.iface.unwrap_or(""),
            self.iface_vers,
            self.name
        )
    }
}

pub struct HandleMethod<'a> {
    iface: Option<&'a str>,
    iface_vers: &'a str,
    name: &'a str,
}

impl<'a> HandleMethod<'a> {
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

impl Display for HandleMethod<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Handle {}[v{}]::{}(...)",
            self.iface.unwrap_or(""),
            self.iface_vers,
            self.name
        )
    }
}
