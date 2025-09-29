use super::error::{oob_err, Result};
use super::handle::Handle;
use super::keypath::{DefaultFromKeypath, Keypath};
use super::load::{LoadKeypath, load_keypath};
use super::store::Store;
use core::fmt::Debug;

#[derive(Debug)]
pub struct Vector<T> {
    len: u64,
    keypath: Keypath,
    // This type should be covariant in T, but (for drop
    // purposes) doesn't actually own values of that type
    _marker: core::marker::PhantomData<fn() -> T>,
}

impl<T> Vector<T> {
    pub fn new(keypath: &Keypath, len: u64) -> Self {
        Self::new_from(keypath.clone(), len)
    }

    pub fn new_from(keypath: Keypath, len: u64) -> Self {
        Self {
            len,
            keypath,
            _marker: core::marker::PhantomData,
        }
    }

    pub fn keypath(&self) -> &Keypath {
        &self.keypath
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: u64) -> Result<T>
    where
        T: LoadKeypath,
    {
        if index >= self.len {
            return oob_err(self.len, index);
        }

        let keypath = self.make_index_keypath(index);

        load_keypath(&keypath)
    }

    pub fn get_default(&self, index: u64) -> Result<T>
    where
        T: LoadKeypath + DefaultFromKeypath,
    {
        if index >= self.len {
            return oob_err(self.len, index);
        }

        let keypath = self.make_index_keypath(index);

        Ok(T::try_load_keypath(&keypath)?
            .unwrap_or_else(|| T::default_from_keypath(&keypath)))
    }

    pub fn load<'a>(&'a self, index: u64) -> Result<ElemRef<'a, T>>
    where
        T: LoadKeypath + DefaultFromKeypath,
    {
        let val = self.get_default(index)?;
        Ok(ElemRef::new(self, val))
    }

    pub fn load_mut<'a>(&'a mut self, index: u64) -> Result<ElemRefMut<'a, T>>
    where
        T: LoadKeypath + DefaultFromKeypath + Store,
    {
        let val = self.get_default(index)?;
        Ok(ElemRefMut::new(self, index, val))
    }

    pub fn get_handle(&self, index: u64) -> Result<Handle<T>>
    where
        T: LoadKeypath + DefaultFromKeypath,
    {
        if index < self.len {
            let keypath = self.make_index_keypath(index);
            let handle = Handle::from(keypath);
            Ok(handle)
        } else {
            oob_err(self.len, index)
        }
    }

    pub fn set(&self, index: u64, val: &T) -> Result<()>
    where
        T: Store,
    {
        if index < self.len {
            let keypath = &self.make_index_keypath(index);
            val.store_keypath(keypath)?;
            Ok(())
        } else {
            oob_err(self.len, index)
        }
    }

    pub fn push(&mut self, val: T) -> Result<()>
    where
        T: Store,
    {
        let new_index = self.len;
        self.len += 1;

        // Store the element value
        self.set(new_index, &val)?;

        // Store ourselves
        self.store_keypath(&self.keypath)?;
        Ok(())
    }

    pub fn extend_len(&mut self, len: u64) -> Result<()>
    where
        T: DefaultFromKeypath,
    {
        if len > self.len {
            self.len = len;
            Ok(())
        } else {
            oob_err(self.len, len) // can only increase length
        }
    }

    fn make_index_keypath(&self, index: u64) -> Keypath {
        let mut keypath = self.keypath.clone();
        keypath.push_index(index);
        keypath
    }

    /*
    fn make_index_key(&self, index: u64) -> StorageKey {
        self.make_index_keypath(index).generate_key()
    }
    */
}

impl<T> DefaultFromKeypath for Vector<T> {
    fn default_from_keypath(keypath: &Keypath) -> Self {
        Vector::new(keypath, 0)
    }
}

const _: () = {
    use xdrs::{WriteXDR, Writer, WriteResult, write_xdr};

    impl<T> WriteXDR for Vector<T> {
        fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult {
            write_xdr(&self.len, w)
        }
    }
};

const _: () = {
    use xdrs::{read_xdr, ReadResult};
    use super::read_xdr_path::*;

    impl<T> ReadXDRPath for Vector<T> {
        fn read_xdr_path<'r, R>(r: &mut R) -> ReadResult<Self>
        where
            R: ReaderPath<'r, Keypath>
        {
            let keypath = r.path();
            let len = read_xdr(r)?;
            Ok(Self::new_from(keypath, len))
        }
    }
};

pub use vref::*;
mod vref {
    use super::*;
    use core::marker::PhantomData;
    use core::ops::{Deref, DerefMut};

    pub struct ElemRef<'a, T> {
        _vec: PhantomData<&'a Vector<T>>,
        val: T,
    }

    impl<'a, T> ElemRef<'a, T> {
        pub fn new(_vec: &'a Vector<T>, val: T) -> Self {
            let _vec = PhantomData;
            Self { _vec, val }
        }
    }

    impl<T> Deref for ElemRef<'_, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.val
        }
    }

    pub struct ElemRefMut<'a, T: Store> {
        vec: &'a mut Vector<T>,
        idx: u64,
        val: T,
    }

    impl<'a, T: Store> ElemRefMut<'a, T> {
        pub fn new(vec: &'a mut Vector<T>, idx: u64, val: T) -> Self {
            Self { vec, idx, val }
        }
    }

    impl<T: Store> Deref for ElemRefMut<'_, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.val
        }
    }

    impl<T: Store> DerefMut for ElemRefMut<'_, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.val
        }
    }

    impl<T: Store> Drop for ElemRefMut<'_, T> {
        fn drop(&mut self) {
            // Flush any changes
            self.vec.set(self.idx, &self.val).unwrap();
        }
    }
}
