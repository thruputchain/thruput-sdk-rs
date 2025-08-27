//! Persist object trees in the KV-store

#![allow(dead_code)]

mod error;
pub use error::*;

mod keypath;
pub use keypath::*;

mod read_path;
pub use read_path::*;

mod load;
use load::*;

mod store;
use store::*;

use core::ops::{Deref, DerefMut};
use core::marker::PhantomData;
use xdrs::{ReadXDR, WriteXDR, Writer, WriteResult};
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

impl<T: LoadPath> Resource<T> {
    fn new(keypath: &Keypath, value: T) -> Self {
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

impl<T: LoadPath> LoadPath for Resource<T> {
    fn load_path(keypath: &Keypath) -> Result<Self> {
        let value = LoadPath::load_path(keypath)?;
        Ok(Resource::new(keypath, value))
    }
}

impl<T: Store> Resource<T> {
    pub fn flush(&mut self) {
        self.value.store(&self.keypath.generate_key()).unwrap_or_else(|e|
            contract_panic!("Resource::flush: {e}"))
    }
}

// --------------------

pub trait LoadPath: Sized {
    fn load_path(keypath: &Keypath) -> Result<Self>;

    fn try_load_path(keypath: &Keypath) -> Result<Option<Self>>
    {
        match Self::load_path(keypath) {
            Ok(r) => Ok(Some(r)),
            Err(e) => if e.is_storage_key_not_exist() {
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}

impl<T: Load> LoadPath for T {
    fn load_path(keypath: &Keypath) -> Result<Self> {
        Load::load(&keypath.generate_key())
    }
}

// -----------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub struct Link<T> {
    keypath: Keypath,
    _marker: PhantomData<fn() -> T>,
}

impl<T> From<Keypath> for Link<T> {
    fn from(keypath: Keypath) -> Self {
        Self {
            keypath,
            _marker: PhantomData
        }
    }
}

impl<T> Link<T> {
    fn new(keypath: &Keypath) -> Self {
        Self::from(keypath.clone())
    }

    pub fn get(&self) -> Resource<T>
        where T: LoadPath
    {
        Resource::load_path(&self.keypath).unwrap_or_else(|e|
            contract_panic!("Handle::get: Resource::load_path: {e}"))
    }

    pub fn put(&mut self, val: &T)
        where T: WriteXDR
    {
        val.store_keypath(&self.keypath).unwrap_or_else(|e|
            contract_panic!("Link::put: {e}"));
    }
}

impl<T> LoadPath for Link<T>
    where T: LoadPath
{
    fn load_path(keypath: &Keypath) -> Result<Self> {
        Ok(keypath.clone().into())
    }
}

impl<T> WriteXDR for Link<T> {
    fn write_xdr<F, W: Writer<F>>(&self, _w: &mut W) -> WriteResult {
        Ok(())
    }
}

// -----------------------------------------------------------------

#[derive(Debug, ReadXDR, WriteXDR, PartialEq)]
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
    fn new(keypath: &Keypath) -> Self {
        Handle::from(keypath.clone())
    }

    pub fn get(&self) -> Resource<T>
        where T: LoadPath
    {
        Resource::load_path(&self.keypath).unwrap_or_else(|e|
            contract_panic!("Handle::get: Resource::load_path: {e}"))
    }

    /*
    pub fn load(&self) -> T
        where T: LoadPath
    {
        T::load_path(&self.keypath).unwrap_or_else(|e|
            contract_panic!("Handle::load failed: {e}"))
    }
    */

    pub fn store(&mut self, value: &T)
        where T: WriteXDR
    {
        let key = self.keypath.generate_key();
        value.store(&key).unwrap_or_else(|e|
            contract_panic!("Handle::store failed: {e}"))
    }

    pub fn root() -> Self {
        Keypath::root().into()
    }
}

pub struct Region {
    name: &'static str,
    i: u64
}

impl Region {
    pub fn new(name: &'static str) -> Self {
        Self { name, i: 0 }
    }

    pub fn alloc<T>(&mut self) -> Handle<T> {
        let mut kp = Keypath::root();
        kp.push_item(self.name.as_bytes());
        kp.push_index(self.i);

        self.i += 1;

        kp.into()
    }
}

pub use hvec::*;
mod hvec {
    use super::*;
    use xdrs::{ReadXDR, WriteXDR};

    #[derive(ReadXDR, WriteXDR)]
    pub struct HVec<T> {
        length: u64,
        _marker: PhantomData<T>,
    }
}

/*
pub use kmap::*;
mod kmap {
    use super::*;

    #[derive(ReadXDR, WriteXDR)]
    pub struct KMap<Key, Val> {
        _marker: PhantomData<fn() -> (Key, Val)>,
    }
}
*/
