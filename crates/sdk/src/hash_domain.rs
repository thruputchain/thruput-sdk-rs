use crate::crypto::SHA256Hash;
use crate::hash::*;
use crate::xdr::*;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, WriteXDR)]
#[repr(i32)]
#[non_exhaustive]
enum ExtensionPoint {
    _0 = 0,
}

impl ExtensionPoint {
    pub const fn new() -> Self {
        Self::_0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, WriteXDR)]
struct DomainSeparator {
    ext: ExtensionPoint,
    domain_separator: BytesM<256>,
}

impl DomainSeparator {
    pub const fn new(domain_separator: BytesM<256>) -> Self {
        Self {
            ext: ExtensionPoint::new(),
            domain_separator,
        }
    }
}

#[cfg(feature = "alloc")]
impl From<&[u8]> for DomainSeparator {
    fn from(bytes: &[u8]) -> Self {
        Self::new(BytesM(bytes.into()))
    }
}

#[cfg(not(feature = "alloc"))]
impl From<&'static [u8]> for DomainSeparator {
    fn from(bytes: &'static [u8]) -> Self {
        Self::new(BytesM(bytes))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, WriteXDR)]
struct HashPreimage<T: WriteXDR> {
    separator: DomainSeparator,
    payload: T,
}

impl<T: WriteXDR> HashPreimage<T> {
    pub fn new(separator: DomainSeparator, payload: T) -> Self {
        Self { separator, payload }
    }
}

/// Generate the hash of the CXS encoding of `(version, domain, payload)`,
/// where `version: u32 = 0`.
#[cfg(feature = "alloc")]
pub fn domain_hash<T: WriteXDR>(domain: &[u8], payload: T) -> SHA256Hash {
    let separator = domain.into();
    let preimage = HashPreimage::new(separator, payload);
    preimage.hash()
}

/// Generate the hash of the CXS encoding of `(version, domain, payload)`,
/// where `version: u32 = 0`.
#[cfg(not(feature = "alloc"))]
pub fn domain_hash<T: WriteXDR>(
    domain: &'static [u8],
    payload: T,
) -> SHA256Hash {
    let separator = domain.into();
    let preimage = HashPreimage::new(separator, payload);
    preimage.hash()
}
