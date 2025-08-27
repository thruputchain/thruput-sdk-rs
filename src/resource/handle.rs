use core::marker::PhantomData;
use xdrs::{ReadXDR, WriteXDR};
use crate::contract_panic;

use super::load::*;
use super::keypath::*;
use super::rsrc::*;
use super::store::*;

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
    /*
    fn new(keypath: &Keypath) -> Self {
        Handle::from(keypath.clone())
    }
    */

    pub fn get(&self) -> Resource<T>
        where T: LoadPath
    {
        Resource::load_path(&self.keypath).unwrap_or_else(|e|
            contract_panic!("Handle::get: Resource::load_path: {e}"))
    }

    pub fn load(&self) -> T
        where T: LoadPath
    {
        T::load_path(&self.keypath).unwrap_or_else(|e|
            contract_panic!("Handle::load failed: {e}"))
    }

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
