//! How a contract method parameter declared as `&T` is decoded from calldata.
//!
//! A `&T` parameter decodes through [`RefArg`], which names what the
//! entrypoint decodes (`Wire`) and how the method's `&T` is taken from it.
//! Types whose wire form is their in-memory form borrow from the calldata
//! buffer and copy nothing. Every other type decodes once into a local, the
//! same single decode a by-value `T` gets, so a `&T` parameter never costs
//! more than `T`. Each type is placed deliberately, never by a fallback that
//! could copy a type that can be borrowed.
//!
//! - Borrowed: `[u8]`, `str`, `[u8; N]`, and the fixed-layout protocol types
//!   (`ContractAddress`, `ContractEntrypointID`, `Ed25519PublicKey`,
//!   `Ed25519Signature`, `SHA256Hash`).
//! - Decoded into a local: every other by-value parameter type, via
//!   [`OwnedRefArg`] (which `#[contract_type]` implements).
//! - Slices `&[T]` of any other element: the elements decode into a local
//!   vector (feature `alloc`). Byte-string elements (`&[&[u8]]`, `&[&str]`)
//!   still borrow their bytes from calldata; only the list of references is
//!   allocated.

use crate::ReceiveAuthHandles;
use crate::xdr::ReadXDR;

/// How a `&Self` contract method parameter is decoded from calldata.
///
/// The entrypoint decodes a [`RefArg::Wire`] with the method's other
/// arguments, then passes [`RefArg::get`] of it as the parameter.
#[diagnostic::on_unimplemented(
    message = "`&{Self}` cannot be a `#[contract]` method parameter",
    label = "no `RefArg` impl for `{Self}`",
    note = "take the parameter by value; a slice `&[T]` of anything but bytes \
            needs the sdk `alloc` feature; a type of your own becomes \
            usable by reference with `#[contract_type]` or an \
            `sdk::call::OwnedRefArg` impl"
)]
pub trait RefArg<'r> {
    /// What the entrypoint decodes from calldata for this parameter: a
    /// reference into the calldata buffer, or an owned value.
    type Wire: ReadXDR<'r> + ReceiveAuthHandles;

    /// The parameter the method receives, borrowed from the decoded value.
    fn get(wire: &Self::Wire) -> &Self;
}

/// Marks a type whose `&Self` contract parameter decodes into an owned
/// local, at the cost of the by-value decode and no more.
///
/// For types whose wire form differs from their in-memory form (integers,
/// vectors, strings, structs), so nothing can point into the calldata.
/// `#[contract_type]` implements it; implement it by hand for a type of
/// your own that is a by-value parameter already. Never implement it for a
/// type that has a borrowing [`RefArg`] impl.
pub trait OwnedRefArg {}

impl<'r, T> RefArg<'r> for T
where
    T: OwnedRefArg + ReadXDR<'r> + ReceiveAuthHandles,
{
    type Wire = T;

    #[inline(always)]
    fn get(wire: &Self::Wire) -> &Self {
        wire
    }
}

/// Marks a type that can be the element of a `&[T; N]` or `&[T]`
/// parameter decoded into a local.
///
/// Every [`OwnedRefArg`] type is one. So are the fixed-size borrowed types
/// (an array or slice of them decodes element by element), and the byte
/// strings `&[u8]` and `&str`, whose bytes stay in calldata. `u8` is not
/// (it has no standalone wire form): `&[u8; N]` and `&[u8]` borrow from
/// calldata directly.
pub trait RefArgElement {}

impl<T: OwnedRefArg> RefArgElement for T {}

/// Borrowed straight from the calldata buffer: the wire form is the
/// in-memory form, so the parameter points into the buffer.
macro_rules! borrowed_ref_arg {
    ($($ty:ty),* $(,)?) => {$(
        impl<'r> RefArg<'r> for $ty {
            type Wire = &'r $ty;

            #[inline(always)]
            fn get(wire: &Self::Wire) -> &Self {
                wire
            }
        }
    )*};
}

borrowed_ref_arg!([u8], str);

/// Borrowed from calldata as parameters, and decoded element by element as
/// the elements of an array or slice parameter.
macro_rules! borrowed_sized_ref_arg {
    ($($ty:ty),* $(,)?) => {$(
        borrowed_ref_arg!($ty);
        impl RefArgElement for $ty {}
    )*};
}

borrowed_sized_ref_arg!(
    crate::types::ContractAddress,
    crate::types::ContractEntrypointID,
    crate::types::Ed25519PublicKey,
    crate::types::Ed25519Signature,
    crate::types::SHA256Hash,
);

impl<'r, const N: usize> RefArg<'r> for [u8; N] {
    type Wire = &'r [u8; N];

    #[inline(always)]
    fn get(wire: &Self::Wire) -> &Self {
        wire
    }
}

impl<const N: usize> RefArgElement for [u8; N] {}
impl RefArgElement for &[u8] {}
impl RefArgElement for &str {}

/// Decoded into a local vector of elements. Byte-string elements borrow
/// their bytes from calldata; only the list of references is allocated.
#[cfg(feature = "alloc")]
impl<'r, T> RefArg<'r> for [T]
where
    T: RefArgElement + ReadXDR<'r> + ReceiveAuthHandles,
{
    type Wire = alloc::vec::Vec<T>;

    #[inline(always)]
    fn get(wire: &Self::Wire) -> &Self {
        wire
    }
}

macro_rules! owned_ref_arg {
    ($($ty:ty),* $(,)?) => {$(
        impl OwnedRefArg for $ty {}
    )*};
}

owned_ref_arg!(
    u16,
    u32,
    u64,
    u128,
    i8,
    i16,
    i32,
    i64,
    i128,
    bool,
    (),
    crate::types::BlockHeight,
    groundhog_u256::U256,
    groundhog_u256::I256,
);

impl<T> OwnedRefArg for Option<T> {}
impl<T: RefArgElement, const N: usize> OwnedRefArg for [T; N] {}
impl<T, const N: usize> OwnedRefArg for heapless::Vec<T, N> {}
impl<const N: usize> OwnedRefArg for heapless::String<N> {}
impl<const MAX: u32> OwnedRefArg for xdrs::BytesM<MAX> {}
impl<T> OwnedRefArg for crate::signer::Signed<T> {}

#[cfg(feature = "alloc")]
impl<T, const MAX: u32> OwnedRefArg for xdrs::VecM<T, MAX> {}

#[cfg(feature = "alloc")]
impl<T> OwnedRefArg for alloc::vec::Vec<T> {}

#[cfg(feature = "alloc")]
impl<T: ?Sized> OwnedRefArg for alloc::boxed::Box<T> {}

macro_rules! owned_ref_arg_tuple {
    ($(($($name:ident),+)),* $(,)?) => {$(
        impl<$($name),+> OwnedRefArg for ($($name,)+) {}
    )*};
}

owned_ref_arg_tuple!(
    (A),
    (A, B),
    (A, B, C),
    (A, B, C, D),
    (A, B, C, D, E),
    (A, B, C, D, E, F),
    (A, B, C, D, E, F, G),
    (A, B, C, D, E, F, G, H),
    (A, B, C, D, E, F, G, H, I),
    (A, B, C, D, E, F, G, H, I, J),
    (A, B, C, D, E, F, G, H, I, J, K),
    (A, B, C, D, E, F, G, H, I, J, K, L),
    (A, B, C, D, E, F, G, H, I, J, K, L, M),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P),
);
