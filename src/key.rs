use crate::StorageKey;

pub enum Key<'a> {
    Ref(&'a StorageKey),
    Own(StorageKey),
}

impl<'a> Key<'a> {
    pub fn as_storage_key(&'a self) -> &'a StorageKey {
        match self {
            Key::Ref(keyref) => *keyref,
            Key::Own(key) => key,
        }
    }
}

pub trait Locator {
    fn as_storage_key(&self) -> &StorageKey;
    fn to_key(&'a self) -> Key<'a>;
}

impl Locator for StorageKey {
    fn as_storage_key(&self) -> &StorageKey {
        self
    }
}


/*
impl<'a> AsRef<StorageKey> for Key<'a> {
    fn as_ref(&self) -> &'a StorageKey {
        match *self {
            Key::Ref(keyref) => keyref,
            Key::Own(key) => &key,
        }
    }
}
*/

/*
pub trait Keyed {
    type Key;

    const fn new(key: Self::Key) -> Self;
}
*/
