use crate::crypto::sha_256;
use crate::types::StorageKey;
use crate::contract_panic;
use xdrs::serialize_cxs::encode as serialize;
use xdrs::{ReadXDR, WriteXDR};
use alloc::{vec, vec::Vec};
use core::fmt::Debug;

#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[xdrs(as_type = "Vec<u8>")]
pub struct Keypath {
    path: Vec<KeypathComponent>,
}

#[derive(Debug, PartialEq, Clone)]
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
                    let v = serialize(i).unwrap_or_else(|e|
                        contract_panic!("Keypath::flattened: serialize: {e}"));
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
        self.push_bytes(serialize(item).unwrap_or_else(|e|
                contract_panic!("Keypath::push_item: serialize: {e}"))
            .as_slice())
    }

    pub fn push_index(&mut self, index: u64) {
        self.path.push(KeypathComponent::Index(index));
    }

    pub fn push_component(&mut self, c: KeypathComponent) {
        self.path.push(c);
    }

    pub fn enter_seq(&mut self) {
        self.push_index(0);
    }

    pub fn seq_next(&mut self) {
        if let Some(KeypathComponent::Index(i)) = self.path.pop() {
            self.path.push(KeypathComponent::Index(i + 1))
        } else {
            panic!("Keypath: seq_next() called when last path component is not an index");
        }
    }

    pub fn leave_seq(&mut self) {
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

impl From<Vec<u8>> for Keypath {
    fn from(bytes: Vec<u8>) -> Self {
        Self {
            path: vec![KeypathComponent::Bytes(bytes)]
        }
    }
}

impl Into<Vec<u8>> for Keypath {
    fn into(self: Keypath) -> Vec<u8> {
        self.flattened()
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

// pub use keycache::*;
mod keycache {
    use super::Keypath;
    use crate::types::StorageKey;
    use core::cell::OnceCell;
    use core::ops::Deref;

    #[derive(Debug)]
    pub struct KeyCache {
        keypath: Keypath,
        key: OnceCell<StorageKey>,
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

        pub fn get(&self) -> &StorageKey {
            self.key.get_or_init(|| self.keypath.generate_key())
        }

        pub fn keypath(&self) -> &Keypath {
            &self.keypath
        }
    }

    impl Deref for KeyCache {
        type Target = StorageKey;

        fn deref(&self) -> &Self::Target {
            self.get()
        }
    }
}
