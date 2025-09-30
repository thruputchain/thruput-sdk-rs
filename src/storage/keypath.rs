use crate::crypto::sha_256;
use crate::types::StorageKey;
use alloc::{vec, vec::Vec};
use core::fmt::Debug;
use xdrs::{WriteXDR, serialize_cxs::encode};

#[derive(Debug, Clone)]
pub struct Keypath {
    path: Vec<KeypathComponent>,
}

#[derive(Debug, Clone)]
pub enum KeypathComponent {
    Bytes(Vec<u8>),
    Index(u64),
}

impl Keypath {
    pub fn new() -> Self {
        Self { path: vec![] }
    }

    pub fn from_key(key: &StorageKey) -> Self {
        let mut keypath = Keypath::new();
        keypath.push_item(key);
        keypath
    }

    pub fn root() -> Self {
        Self::new()
    }

    pub fn clone_flattened(&self) -> Self {
        Keypath {
            path: vec![KeypathComponent::Bytes(self.flattened())],
        }
    }

    pub fn flattened(&self) -> Vec<u8> {
        let mut out = vec![];
        for item in self.path.iter() {
            match item {
                KeypathComponent::Bytes(v) => {
                    out.extend_from_slice(v.as_slice());
                }
                KeypathComponent::Index(i) => {
                    let v = encode(i).unwrap();
                    out.extend_from_slice(v.as_slice());
                }
            }
        }
        out
    }

    pub fn generate_key(&self) -> StorageKey {
        sha_256(self.flattened().as_slice()).unwrap().into()
    }

    fn push_bytes(&mut self, slice: &[u8]) {
        self.path.push(KeypathComponent::Bytes(Vec::from(slice)))
    }

    pub fn push_item<I: WriteXDR + ?Sized>(&mut self, item: &I) {
        self.push_bytes(encode(item).unwrap().as_slice())
    }

    pub fn push_index(&mut self, index: u64) {
        self.path.push(KeypathComponent::Index(index));
    }

    pub fn seq_begin(&mut self) {
        self.push_index(0);
    }

    pub fn seq_next(&mut self) {
        if let Some(KeypathComponent::Index(i)) = self.path.pop() {
            self.path.push(KeypathComponent::Index(i + 1))
        } else {
            panic!("Keypath: seq_next() called when last path component is not an index");
        }
    }

    pub fn seq_end(&mut self) {
        if !matches!(self.path.pop(), Some(KeypathComponent::Index(_))) {
            panic!("Keypath: leave_seq() called when last path component is not an index");
        }
    }
}

impl Default for Keypath {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn flattening_conserves_key() {
        use super::*;

        let mut kp = Keypath::new();
        kp.push_item("some bytes");
        kp.push_index(3);

        let kp2 = kp.clone_flattened();

        assert_eq!(kp.flattened(), kp2.flattened())
    }
}

pub use keycache::*;
mod keycache {
    use super::Keypath;
    use crate::types::StorageKeyContractData;
    use core::cell::OnceCell;
    use core::ops::Deref;

    #[derive(Debug)]
    pub struct KeyCache {
        keypath: Keypath,
        key: OnceCell<StorageKeyContractData>,
    }

    impl From<Keypath> for KeyCache {
        fn from(keypath: Keypath) -> Self {
            KeyCache {
                keypath,
                key: OnceCell::new(),
            }
        }
    }

    impl KeyCache {
        pub fn new(keypath: &Keypath) -> Self {
            Self {
                keypath: keypath.clone_flattened(),
                key: OnceCell::new(),
            }
        }

        pub fn get(&self) -> &StorageKeyContractData {
            self.key.get_or_init(|| self.keypath.generate_key())
        }

        pub fn keypath(&self) -> &Keypath {
            &self.keypath
        }
    }

    impl Deref for KeyCache {
        type Target = StorageKeyContractData;

        fn deref(&self) -> &Self::Target {
            self.get()
        }
    }
}
