//! Persist object trees in the KV-store

#![allow(dead_code)]

mod keypath;
use keypath::*;

pub use kref::*;
pub use kvec::*;
pub use kmap::*;

use core::ops::{Deref, DerefMut};
use core::marker::PhantomData;
use xdrs::{ReadXDR, WriteXDR};

struct K<T> {
    keypath: Keypath,
    value: T,
}

impl<T> Deref for K<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for K<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

mod kref {
    use super::*;

    #[derive(ReadXDR, WriteXDR)]
    pub struct KRef<T> {
        _marker: PhantomData<fn() -> T>,
    }
}

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
