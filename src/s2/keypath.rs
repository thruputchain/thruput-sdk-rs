use crate::types::StorageKey;
use crate::hex::*;
use crate::{sha256_array, crypto::sha256};
use core::fmt::{self, Display, Debug};
use xdrs::{WriteXDR, serialize_cxs::encode};
use heapless::Vec;
use thiserror::Error;

/*
// Key

#[derive(Clone)]
pub struct Key(KeyBuf);

pub enum Key2 {
    Path(PathBuf),
    SK(StorageKey),
}
*/

const PATH_BUF_SIZE: usize = 100;

pub type PathBuf = Vec<u8, PATH_BUF_SIZE>;
pub type KeyBuf = Vec<u8, 32>;

// Keypath serialization

#[derive(Debug, Error)]
#[error("Capacity or XDR-encoding error")]
enum SerializeFailure {
    #[error("Capacity error")]
    Capacity,

    #[error("Encoding error: {0}")]
    Encoding(#[from] xdrs::WriteError),
}

type SerializeResult = Result<(), SerializeFailure>;

impl From<u8> for SerializeFailure {
    fn from(_: u8) -> Self {
        Self::Capacity
    }
}

impl From<heapless::CapacityError> for SerializeFailure {
    fn from(_: heapless::CapacityError) -> Self {
        Self::Capacity
    }
}

// Step: a component of a Keypath

type StepBuf = Vec<u8, 100>;

#[derive(Debug, Clone)]
enum Step {
    Key(StorageKey),
    Bytes(StepBuf),
    Index(u64),
    Nil,
}

impl<'a> Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Key(key) => write!(f, "k:{}", hex_encode(key)),
            Self::Bytes(buf) => write!(f, "b:[{}]", hex_encode(buf)),
            Self::Index(i) => write!(f, "i:{}", i),
            Self::Nil => Ok(()),
        }
    }
}

impl Step {
    pub fn serialize_into(&self, dest: &mut PathBuf) -> SerializeResult {
        match self {
            Self::Nil => {},
            Self::Key(key) => {
                dest.push(b'k')?;

                dest.extend_from_slice(key.as_ref())?;
            }
            Self::Bytes(buf) => {
                dest.push(b'b')?;

                let len = (buf.len() as u8).to_be_bytes();
                dest.extend_from_slice(&len[..])?;

                dest.extend_from_slice(buf)?;
            }
            Self::Index(i) => {
                dest.push(b'i')?;

                dest.extend_from_slice(encode(i)?.as_slice())?;
            }
        }
        Ok(())
    }
}

// Keypath //

pub struct Keypath<'a> {
    head: Step,
    tail: Option<&'a Keypath<'a>>,
}

impl Default for Keypath<'_> {
    fn default() -> Self { Keypath { head: Step::Nil, tail: None } }
}

impl From<StorageKey> for Keypath<'_> {
    fn from(key: StorageKey) -> Self {
        Keypath {
            head: Step::Key(key),
            tail: None,
        }
    }
}

impl<'a> Display for Keypath<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(tail) = self.tail {
            Display::fmt(tail, f)?;
            f.write_str(" / ")?;
        }
        Display::fmt(&self.head, f)
    }
}

impl<'a> Keypath<'a> {
    #[deprecated = "Use Keypath::new"]
    pub fn root() -> Self {
        Self::default()
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn generate_key(&self) -> StorageKey {
        let buf = self.serialize();

        // If the serialized keypath (plus length byte) already fits in a
        // StorageKey, then use that directly.
        {
            let len = buf.len();
            if len <= 31 {
                const SDK_STORAGE_KEY_DEFAULT: [u8; 32] = sha256_array!("SDK_STORAGE_KEY_DEFAULT");

                // Start with a known pattern
                let mut key: [u8; 32] = SDK_STORAGE_KEY_DEFAULT;

                // Put the length into the first byte
                key[0] = len as u8;

                // Append the keypath contents
                key[1..len+1].copy_from_slice(&buf[0..len]);

                return key.into();
            }
        }

        sha256(buf).into()
    }

    fn serialize_into(&self, buf: &mut PathBuf) -> SerializeResult {
        self.head.serialize_into(buf)?;

        if let Some(tail) = self.tail {
            tail.serialize_into(buf)?;
        }

        Ok(())
    }

    pub fn serialize(&self) -> PathBuf {
        let mut buf = Vec::new();

        self.serialize_into(&mut buf).unwrap_or_else(|e|
            panic!("{}", e));

        buf
    }

    fn to_tail(&'a self) -> Option<&'a Keypath<'a>> {
        if matches!(self.head, Step::Nil) {
            None
        } else {
            Some(self)
        }
    }

    fn with(&'a self, step: Step) -> Keypath<'a> {
        Keypath {
            head: step,
            tail: self.to_tail(),
        }
    }

    pub fn with_index(&'a self, i: u64) -> Keypath<'a> {
        self.with(Step::Index(i))
    }

    pub fn with_bytes(&'a self, slice: &[u8]) -> Keypath<'a> {
        assert!(slice.len() < 256);

        self.with(Step::Bytes(StepBuf::from_slice(slice).unwrap()))
    }

    pub fn with_item<I: WriteXDR + ?Sized>(&'a self, item: &I) -> Keypath<'a> {
        self.with_bytes(encode(item).unwrap().as_slice())
    }

    pub fn with_key(&'a self, key: StorageKey) -> Keypath<'a> {
        self.with(Step::Key(key))
    }
}

// DefaultFromKeypath

pub trait DefaultFromKeypath {
    fn default_from_keypath(keypath: &Keypath) -> Self;
}

impl<T> DefaultFromKeypath for T
where
    T: Default,
{
    fn default_from_keypath(_: &Keypath) -> Self {
        Default::default()
    }
}

#[cfg(test)]
mod test_keypath2 {
    use super::*;

    macro_rules! show {
        { let $var:ident = $value:expr; } => {
            let $var = $value;

            std::eprintln!("let {} = {{ {} }};\n\
                <{}> -> [{}]\n",
                stringify!($var),
                $var,
                hex_encode(&$var.serialize()),
                hex_encode(&$var.generate_key()),
                );
        };
    }

    #[test]
    fn check_keypaths() {
        show!{ let root = Keypath::new(); }

        show!{ let dict = root.with_index(0); }

        show!{ let dict_foo = dict.with_bytes(b"foo"); }

        show!{ let vec = root.with_index(1); }

        show!{ let vec_23 = vec.with_index(23); }

        show!{ let keyed = root.with_key(StorageKey::default()); }

        show!{ let keyed_bar = keyed.with_bytes(b"bar"); }
    }
}
