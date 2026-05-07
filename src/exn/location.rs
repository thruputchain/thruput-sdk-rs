#![allow(dead_code)]

use crate::xdr::*;

/// A container that occupies zero memory when feature "debugging" is disabled,
/// and serializes compatibly with code where the feature is enabled.
///
/// Encoding: (true)(opaque_variable_encoding_of_inner) | (false)
///
#[cfg(feature = "debugging")]
mod inner {
    #[derive(Debug)]
    #[repr(i32)]
    pub enum DebuggingOnly<T> {
        Empty,
        Full(T),
    }

    impl<T> DebuggingOnly<T> {
        fn empty() -> Self {
            Self::Empty
        }

        fn unwrap(self) -> Option<T> {
            match self {
                Self::Empty => None,
                Self::Full(v) => Some(v),
            }
        }
    }

    impl<T> From<T> for DebuggingOnly<T> {
        fn from(v: T) -> Self {
            Self::Full(v)
        }
    }
}

#[cfg(not(feature = "debugging"))]
mod inner {
    #[derive(Debug)]
    pub struct DebuggingOnly<T> {
        phantom: core::marker::PhantomData<T>,
    }

    impl<T> DebuggingOnly<T> {
        pub fn empty() -> Self {
            Self {
                phantom: core::marker::PhantomData,
            }
        }
    }
}

pub use inner::*;

#[cfg(feature = "debugging")]
impl<'r, T> ReadXDR<'r> for DebuggingOnly<T>
where
    T: ReadXDROwned,
{
    #[inline]
    fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
        let present: bool = read_xdr(r)?;

        if present {
            let encoding = r.read_opaque_variable(u32::MAX)?;
            let inner = cxs::decode(encoding)?;
            Ok(Self::Full(inner))
        } else {
            Ok(Self::Empty)
        }
    }
}

#[cfg(not(feature = "debugging"))]
impl<'r, T> ReadXDR<'r> for DebuggingOnly<T> {
    #[inline]
    fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
        let present: bool = read_xdr(r)?;

        if present {
            // Ignore the encoded value
            let _encoding = r.read_opaque_variable(u32::MAX)?;
        }

        Ok(Self::empty())
    }
}

#[cfg(feature = "debugging")]
impl<T> WriteXDR for DebuggingOnly<T>
where
    T: WriteXDR,
{
    #[inline]
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        match self {
            Self::Empty => false.write_xdr(w),
            Self::Full(x) => {
                true.write_xdr(w)?;
                let mut buf = heapless::Vec::<u8, 1000>::new();
                let written = cxs::encode_into(x, buf.spare_capacity_mut())?;
                unsafe {
                    buf.set_len(written);
                }
                buf.write_xdr(w)
            }
        }
    }
}

#[cfg(not(feature = "debugging"))]
impl<T> WriteXDR for DebuggingOnly<T> {
    #[inline]
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        false.write_xdr(w)
    }
}

#[derive(ReadXDR, WriteXDR, Debug)]
pub struct Location {
    file: heapless::String<200>,
    line: u32,
}

#[cfg(feature = "debugging")]
impl<'a> From<&'a core::panic::Location<'a>> for Location {
    fn from(loc: &core::panic::Location) -> Self {
        Self {
            file: loc.file().try_into().unwrap(), // XXX: allow truncated string
            line: loc.line(),
        }
    }
}

/*
use crate::xdr::*;

#[derive(Debug, ReadXDR, WriteXDR)]
#[repr(i32)]
#[xdrs(from_type = "RawLocation<'_>", into_type = "RawLocation<'_>")]
pub enum Location {
    Unknown,

    #[cfg(feature = "debugging")]
    Details(LocationDetails),
}

#[cfg(feature = "debugging")]
#[derive(Debug, ReadXDR, WriteXDR)]
pub struct LocationDetails {
    file: heapless::String<200>,
    line: u32,
}

///////////////////////////////////////

#[derive(Debug, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum RawLocation<'a> {
    Unknown,
    Encoded(&'a [u8]),
}

type DetailsBuf = heapless::Vec<u8, 204>;

impl<'a> RawLocation<'a> {
    pub fn unknown() -> Self {
        Self::Unknown
    }
}

impl From<RawLocation<'_>> for Location {
    fn from(raw: RawLocation<'_>) -> Self {
        match raw {
            RawLocation::Unknown => Location::Unknown,
            RawLocation::Encoded(_buf) => {
                todo!();
            }
        }
    }
}



impl From<Location> for RawLocation {
    fn from(loc: Location) -> Self {
        match loc {
            Location::Unknown => RawLocation::Unknown,
            Location::Details(details) => {
                let mut buf = DetailsBuf::new();
                match cxs::encode_into(&details, buf.spare_capacity_mut()) {
                    Ok(written) => {
                        unsafe { buf.set_len(written); }
                        RawLocation::Encoded(buf)
                    }
                    _ => RawLocation::Unknown
                }
            }
        }
    }
}

/*
impl<'a> From<&'a core::panic::Location<'a>> for RawLocation {
    fn from(loc: &core::panic::Location) -> Self {
        let repr = Location::from(loc);
        repr.into()
    }
}
*/
*/
