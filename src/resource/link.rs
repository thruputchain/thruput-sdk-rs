use super::error::*;
use super::load::*;
use super::keypath::*;
use super::rsrc::*;
use super::store::*;

use core::marker::PhantomData;
use xdrs::{WriteXDR, Writer, WriteResult};
use crate::contract_panic;

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
    /*
    fn new(keypath: &Keypath) -> Self {
        Self::from(keypath.clone())
    }
    */

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
