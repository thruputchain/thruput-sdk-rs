use core::fmt::{self, Debug, Display};
use core::any::Any;
use core::error::Error;
use crate::xdr::*;

#[cfg(feature = "fault_strings")]
use heapless::String;

/// A fault from a contract.
///
// TODO: Figure out how to make this serialization-compatible whether
// the feature is enabled or not.
//
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
pub struct FaultString(
    #[cfg(feature = "fault_strings")]
    String<500>
);

impl Error for FaultString {}

impl Display for FaultString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FaultString {
    pub fn as_str(&self) -> &str {
        #[cfg(feature = "fault_strings")]
        {
            self.0.as_str()
        }

        #[cfg(not(feature = "fault_strings"))]
        {
            "Fault"
        }
    }

    pub fn via_display<V>(v: &V) -> Self
    where
        V: Display + ?Sized,
    {
        #[cfg(feature = "fault_strings")]
        {
            use core::fmt::Write;

            let mut s = String::new();
            write!(s, "{}", v).unwrap_or_else(|_| {
                s.clear();
                let _ = write!(s, "<too-big>");
            });

            Self(s)
        }

        #[cfg(not(feature = "fault_strings"))]
        {
            let _ = v;

            Self()
        }
    }

    pub fn from_display(s: impl Display) -> Self {
        Self::via_display(&s)
    }

    pub fn from_any(a: &dyn Any) -> Self {
        #[cfg(feature = "fault_strings")]
        {
            if let Some(s) = a.downcast_ref::<&str>() {
                Self::from_display(s)
                    /*
            } else if let Some(s) = a.downcast_ref::<&std::string::String>() {
                Self::via_display(s.as_str())
                    */
            } else {
                Self::from_display("<unknown>")
            }
        }

        #[cfg(not(feature = "fault_strings"))]
        {
            let _ = a;

            Self()
        }
    }
}
