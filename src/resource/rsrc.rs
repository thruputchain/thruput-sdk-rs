use core::ops::{Deref, DerefMut};
use crate::contract_panic;

use super::error::*;
use super::handle::*;
use super::load::*;
use super::store::*;
use super::keypath::*;

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
