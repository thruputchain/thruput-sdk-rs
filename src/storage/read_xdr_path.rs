use super::Keypath;
use delegate::delegate;
use xdrs::*;

pub trait PathHandler {
    type Path;

    fn path(&self) -> Self::Path;

    fn seq_begin(&mut self);
    fn seq_next(&mut self);
    fn seq_end(&mut self);
}

pub trait ReaderPath<'r, P>: Reader<'r> + PathHandler<Path = P> {}

//////////////////////////////////////////////////////////////

pub struct KeypathReader<R> {
    pub xdr_reader: R,
    pub path: Keypath,
}

impl<'r, R> PathHandler for KeypathReader<R>
where
    R: Reader<'r>,
{
    type Path = Keypath;

    fn path(&self) -> Self::Path {
        self.path.clone()
    }

    delegate! {
        to self.path {
            fn seq_begin(&mut self);
            fn seq_next(&mut self);
            fn seq_end(&mut self);
        }
    }
}

impl<'r, R> Reader<'r> for KeypathReader<R>
where
    R: Reader<'r>,
{
    delegate! {
        to self.xdr_reader {
            fn read_i64(&mut self) -> ReadResult<i64>;
            fn read_i32(&mut self) -> ReadResult<i32>;
            fn read_i16(&mut self) -> ReadResult<i16>;

            fn read_u64(&mut self) -> ReadResult<u64>;
            fn read_u32(&mut self) -> ReadResult<u32>;
            fn read_u16(&mut self) -> ReadResult<u16>;

            fn read_opaque_fixed(&mut self, len: u32) -> ReadResult<&'r [u8]>;
            fn read_opaque_variable(&mut self, max: u32) -> ReadResult<&'r [u8]>;
        }
    }
}

impl<'r, R> ReaderPath<'r, Keypath> for KeypathReader<R> where R: Reader<'r> {}

//////////////////////////////////////////////////////////////

pub trait ReadXDRPath: Sized {
    fn read_xdr_path<'r, R>(r: &mut R) -> ReadResult<Self>
    where
        R: ReaderPath<'r, Keypath>;
}

impl<T> ReadXDRPath for T
where
    T: for<'a> ReadXDR<'a>,
{
    fn read_xdr_path<'r, R>(r: &mut R) -> ReadResult<Self>
    where
        R: ReaderPath<'r, Keypath>,
    {
        ReadXDR::read_xdr(r)
    }
}

/*
macro_rules! delegate_read_xdr_path {
    { $($ty:ty),* $(,)? } => { $(

        impl ReadXDRPath for $ty
        {
            fn read_xdr_path<'r, R>(r: &mut R) -> ReadResult<Self>
            where
                R: ReaderPath<'r, Keypath>,
            {
                ReadXDR::read_xdr(r)
            }
        }

    )* };
}

delegate_read_xdr_path! {
    (), i32, u32, i64, u64, Vec<u8>,
}
*/

/*
impl<T, E> ReadXDRPath for Result<T, E>
where
    T: ReadXDRPath,
    E: ReadXDRPath,
{
    fn read_xdr_path<'r, R>(r: &mut R) -> ReadResult<Self>
    where
        R: ReaderPath<'r, Keypath>,
    {
        let d: u32 = <u32 as ReadXDR>::read_xdr(r)?;

        let result = if d > 0 {
            Ok(<T as ReadXDRPath>::read_xdr_path(r)?)
        } else {
            Err(<E as ReadXDRPath>::read_xdr_path(r)?)
        };

        Ok(result)
    }
}
*/
