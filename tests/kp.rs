use sdk::types::StorageKey;
use sdk::hex::*;
use core::fmt::{self, Display, Debug};
use xdrs::{WriteXDR, serialize_cxs::encode};
use heapless::Vec;
use thiserror::Error;

const PATH_BUF_SIZE: usize = 100;

type PathBuf = Vec<u8, PATH_BUF_SIZE>;

type StepBuf = Vec<u8, 256>;

#[allow(unused)]
fn sha256<T: AsRef<[u8]>>(buf: T) -> [u8; 32] {
    use sha2::{Sha256, Digest};

    Sha256::digest(buf.as_ref()).into()
}

#[derive(Debug, Clone)]
pub struct Keypath<'a> {
    head: Step,
    tail: Option<&'a Keypath<'a>>,
}

impl<'a> Display for Keypath<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(tail) = self.tail {
            Display::fmt(tail, f)?;
            f.write_str(" -> ")?;
        }

        Display::fmt(&self.head, f)?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
enum Step {
    Key(StorageKey),
    Bytes(StepBuf),
    Index(u64),
}

impl<'a> Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Key(key) => write!(f, "k:{}", 0u8 /* hex_encode(key) XXX */),
            Self::Bytes(buf) => write!(f, "b<{}>:{}", buf.len(), hex_encode(buf)),
            Self::Index(i) => write!(f, "i<{}>", i),
        }
    }
}

#[derive(Debug, Error)]
#[error("Capacity error serializing keypath")]
struct SerializeFailure;

type SerializeResult = Result<(), SerializeFailure>;

impl From<u8> for SerializeFailure {
    fn from(_: u8) -> Self { Self }
}

impl From<heapless::CapacityError> for SerializeFailure {
    fn from(_: heapless::CapacityError) -> Self { Self }
}

impl From<xdrs::WriteError> for SerializeFailure {
    fn from(_: xdrs::WriteError) -> Self { Self }
}

impl Step {
    pub fn serialize(&self, dest: &mut PathBuf) -> SerializeResult {
        match self {
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

impl<'a> Keypath<'a> {
    pub fn root() -> Self {
        Self::default()
    }
    pub fn new() -> Self {
        Self::default()
    }

    pub fn generate_key(&self) -> StorageKey {
        // sha256(self.flattened().as_slice()).into()

        let mut key = [0; 32];
        let raw = self.flattened();
        let len = raw.len().clamp(0, 32);
        &key[0..len].copy_from_slice(&raw[0..len]);
        key.into()
    }

    fn serialize(&self, buf: &mut PathBuf) -> SerializeResult {
        // XXX: write in order to avoid stack exhaustion
        if let Some(tail) = self.tail {
            tail.serialize(buf)?;
        }

        self.head.serialize(buf)
    }

    pub fn flattened(&self) -> PathBuf {
        let mut buf = Vec::new();

        self.serialize(&mut buf).unwrap_or_else(|e|
            panic!("{}", e));

        buf
    }

    fn with(&'a self, step: Step) -> Keypath<'a> {
        let head = step;
        let tail = Some(self);
        Keypath { head, tail }
    }

    pub fn with_bytes(&'a self, slice: &[u8]) -> Keypath<'a> {
        assert!(slice.len() < 256);

        self.with(Step::Bytes(StepBuf::from_slice(slice).unwrap()))
    }

    pub fn with_item<I: WriteXDR + ?Sized>(&'a self, item: &I) -> Keypath<'a> {
        self.with_bytes(encode(item).unwrap().as_slice())
    }

    pub fn with_index(&'a self, i: u64) -> Keypath<'a> {
        self.with(Step::Index(i))
    }
}

impl From<StorageKey> for Keypath<'_> {
    fn from(key: StorageKey) -> Self {
        let head = Step::Key(key);
        let tail = None;

        Keypath { head, tail }
    }
}

impl Default for Keypath<'_> {
    fn default() -> Self {
        StorageKey::default().into()
    }
}

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
mod test_keypath {
    use super::*;

    macro_rules! show {
        { let $var:ident = $value:expr; } => {
            let $var = $value;

            eprintln!("let {} = {{ {} }};\n{}\n",
                stringify!($var),
                $var,
                hex_encode(&$var.flattened()),
                );
        };
    }

    #[test]
    fn check_keypaths() {
        show!{ let root = Keypath::root(); }

        show!{ let dict = root.with_index(0); }

        show!{ let dict_foo = dict.with_bytes(b"foo"); }

        show!{ let vec = root.with_index(1); }

        show!{ let vec_23 = vec.with_index(23); }
    }
}
