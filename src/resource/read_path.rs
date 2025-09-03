use xdrs::{ReadXDR, BytesM};
use xdrs::serialize_cxs::buf_reader::*;
use xdrs::read_xdr;
use super::keypath::*;
use super::error::*;
use super::link::*;
use super::handle::*;

pub fn read_path<'r, T>(keypath: &mut Keypath, reader: &mut BufReader<'r>) -> Result<T>
    where T: ReadPath<'r>
{
    let precursor = read_xdr(reader)?;
    let value = T::from_path_via(keypath, precursor);
    Ok(value)
}

pub trait ReadPath<'r>: Sized {
    // An XDR-mapped type that can be combined with a keypath to produce Self
    type Precursor: ReadXDR<'r>;

    fn from_path_via(keypath: &Keypath, precursor: Self::Precursor) -> Self;
}

impl<'r, T: ReadPath<'r>> ReadPath<'r> for Link<T> {
    type Precursor = ();

    // Link uses the implicit, structural keypath
    fn from_path_via(keypath: &Keypath, _: Self::Precursor) -> Self {
        keypath.clone().into()
    }
}

impl<'r, T> ReadPath<'r> for Handle<T> {
    type Precursor = Self;

    // Handle uses a stored keypath
    fn from_path_via(_: &Keypath, precursor: Self::Precursor) -> Self {
        precursor
    }
}

impl<'r, T> ReadPath<'r> for Option<T>
    where T: ReadPath<'r>
{
    type Precursor = Option<T::Precursor>;

    fn from_path_via(keypath: &Keypath, precursor: Self::Precursor) -> Self {
        precursor.map(|p| T::from_path_via(keypath, p))
    }
}

// TODO: container impls:
// array, array-ref, slice, core::marker::PhantomData,
// (alloc) string, Vec
// (xdr_common) VecM

macro_rules! impl_read_path_via_read_xdr {
    { $($ty:ty),* $(,)? } => { $(
        // Just ignore the path and return the result of read_xdr
        impl<'r> ReadPath<'r> for $ty
        {
            type Precursor = $ty;

            fn from_path_via(_: &Keypath, precursor: Self::Precursor) -> Self {
                precursor
            }
        }
    )* };
}

impl_read_path_via_read_xdr! {
    (),
    i16, u16,
    i32, u32,
    i64, u64,
    bool,
    BytesM,
}

macro_rules! impl_read_path_tuple {
    { $( $t:ident ),+ } => {

        impl<'r, $($t),+> ReadPath<'r> for ($($t),+)
        where
            $( $t: ReadPath<'r>, )+
        {
            type Precursor = ($($t::Precursor),+);

            #[allow(non_snake_case)]
            fn from_path_via(keypath: &Keypath, precursor: Self::Precursor) -> Self {
                let ($($t),+) = precursor;
                ( $(ReadPath::from_path_via(keypath, $t)),+ )
            }
        }

    };
}

impl_read_path_tuple!(T1, T2);
impl_read_path_tuple!(T1, T2, T3);
impl_read_path_tuple!(T1, T2, T3, T4);
impl_read_path_tuple!(T1, T2, T3, T4, T5);
impl_read_path_tuple!(T1, T2, T3, T4, T5, T6);
