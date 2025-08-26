//! Persist object trees in the KV-store

#![allow(dead_code)]

mod error;

mod load;
use load::*;

mod store;
use store::*;

mod keypath;
pub use keypath::*;

use core::ops::{Deref, DerefMut};
use core::marker::PhantomData;
use xdrs::{ReadXDROwned, WriteXDR};
use crate::contract_panic;

pub struct Resource<T> {
    keypath: Keypath,
    value: T,
}

impl<T> Deref for Resource<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for Resource<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T> Resource<T> {
    pub fn new(keypath: &Keypath, value: T) -> Self {
        Self {
            keypath: keypath.clone(),
            value,
        }
    }

    pub fn keypath(&self) -> &Keypath {
        &self.keypath
    }

    pub fn get_handle<U>(&self, kpc: KeypathComponent) -> Handle<U> {
        let mut kp = self.keypath().clone_flattened();
        kp.push_component(kpc);
        kp.into()
    }
}

pub struct Handle<T> {
    keypath: Keypath,
    _marker: PhantomData<fn() -> T>,
}

impl<T> From<Keypath> for Handle<T> {
    fn from(keypath: Keypath) -> Self {
        Self {
            keypath,
            _marker: PhantomData
        }
    }
}

impl<T> Handle<T> {
    pub fn new(keypath: &Keypath) -> Self {
        Handle::from(keypath.clone())
    }

    pub fn load(&self) -> T
        where T: ReadXDROwned
    {
        let key = self.keypath.generate_key();
        T::load(&key).unwrap_or_else(|e|
            contract_panic!("Handle::load failed: {e}"))
    }

    pub fn store(&mut self, value: T)
        where T: WriteXDR
    {
        let key = self.keypath.generate_key();
        value.store(&key).unwrap_or_else(|e|
            contract_panic!("Handle::store failed: {e}"))
    }
}

/*
pub use kvec::*;
pub use kmap::*;

mod kvec {
    use super::*;

    #[derive(ReadXDR, WriteXDR)]
    pub struct KVec<T> {
        length: u64,
        _marker: PhantomData<T>,
    }
}

mod kmap {
    use super::*;

    #[derive(ReadXDR, WriteXDR)]
    pub struct KMap<Key, Val> {
        _marker: PhantomData<fn() -> (Key, Val)>,
    }
}
*/
