use core::fmt::{self, Display};
use sdk::*;

#[contracttype]
#[derive(Debug)]
pub struct Name {
    pub inner: xdr_common::BytesM<100>,
}

impl TryFrom<&str> for Name {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, ()> {
        let inner = s.try_into().map_err(|_| ())?;
        Ok(Self { inner })
    }
}

#[cfg(feature = "dev")]
impl From<Name> for String {
    fn from(v: Name) -> Self {
        String::from_utf8(v.inner.0).unwrap()
    }
}

impl Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

#[contracttype]
#[derive(Debug)]
pub struct Symbol {
    pub inner: xdr_common::BytesM<10>,
}

impl TryFrom<&str> for Symbol {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, ()> {
        let inner = s.try_into().map_err(|_| ())?;
        Ok(Self { inner })
    }
}

#[cfg(feature = "dev")]
impl From<Symbol> for String {
    fn from(v: Symbol) -> Self {
        String::from_utf8(v.inner.0).unwrap()
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}
