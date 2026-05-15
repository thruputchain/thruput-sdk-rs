use crate::crypto::sha256;
use crate::types::StorageKey;
use alloc::{vec, vec::Vec};
use core::fmt::Debug;
use xdrs::{WriteXDR, cxs::encode};

#[derive(Debug, Clone)]
pub struct Keypath {
    path: Vec<KeypathComponent>,
    exclusive: bool,
}

#[derive(Debug, Clone)]
pub enum KeypathComponent {
    Bytes(Vec<u8>),
    Index(u64),
}

impl Keypath {
    pub fn new() -> Self {
        Self {
            path: vec![],
            exclusive: false,
        }
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
            exclusive: self.exclusive,
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
        sha256(self.flattened().as_slice()).into()
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
            panic!(
                "Keypath: seq_next() called when last path component is not an index"
            );
        }
    }

    pub fn seq_end(&mut self) {
        if !matches!(self.path.pop(), Some(KeypathComponent::Index(_))) {
            panic!(
                "Keypath: leave_seq() called when last path component is not an index"
            );
        }
    }

    pub fn is_exclusive(&self) -> bool {
        self.exclusive
    }

    pub fn set_exclusive(&mut self) {
        self.exclusive = true;
    }
}

impl Default for Keypath {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn exclusive_flag_propagates_through_clone() {
        use super::*;

        let mut kp = Keypath::new();
        kp.push_item("some bytes");
        kp.set_exclusive();

        let kp2 = kp.clone();
        assert!(kp2.is_exclusive());

        let kp3 = kp.clone_flattened();
        assert!(kp3.is_exclusive());
    }

    #[test]
    fn exclusive_flag_defaults_to_false() {
        let kp = super::Keypath::new();
        assert!(!kp.is_exclusive());
    }
}

pub use keycache::*;
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
