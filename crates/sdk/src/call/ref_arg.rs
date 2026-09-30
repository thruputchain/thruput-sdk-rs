//! How a contract method parameter declared as `&T` is decoded from calldata.
//!
//! A `#[contract]` entrypoint decodes its arguments from the calldata buffer
//! and then calls the method. An owned parameter `T` decodes as `T`. A
//! reference parameter `&T` goes through [`RefArg`], which names what is
//! decoded (`Wire`) and how the method's `&T` is taken from it.
//!
//! Types whose wire form is their in-memory form (fixed-size byte arrays and
//! the fixed-layout protocol types built on them) decode as a reference into
//! the calldata buffer, so passing them by reference copies nothing. Types
//! whose wire form differs from their in-memory form (a length-prefixed
//! vector) cannot be borrowed from the buffer: they decode into an owned
//! local, and the method receives a reference to that local, the same single
//! decode an owned parameter gets.
//!
//! Each supported type has its own impl, so a type is borrowed from the
//! buffer or decoded into a local by choice, never by a blanket fallback
//! that could add a copy to a type that can be borrowed.

use crate::ReceiveAuthHandles;
use crate::xdr::ReadXDR;

/// How a `&Self` contract method parameter is decoded from calldata.
///
/// The entrypoint decodes a [`RefArg::Wire`] with the method's other
/// arguments, then passes [`RefArg::get`] of it as the parameter. A type
/// without an impl cannot be taken by reference in a `#[contract]` method;
/// take it by value instead.
#[diagnostic::on_unimplemented(
    message = "`&{Self}` cannot be a `#[contract]` method parameter",
    label = "no `RefArg` impl for `{Self}`",
    note = "take the parameter by value, or add a `sdk::call::RefArg` impl \
            for `{Self}`"
)]
pub trait RefArg<'r> {
    /// What the entrypoint decodes from calldata for this parameter: a
    /// reference into the calldata buffer, or an owned value.
    type Wire: ReadXDR<'r> + ReceiveAuthHandles;

    /// The parameter the method receives, borrowed from the decoded value.
    fn get(wire: &Self::Wire) -> &Self;
}

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

// Exactly the types a contract could already take by reference: those with
// a borrowed `ReadXDR` (a view into the calldata buffer) and a
// `ReceiveAuthHandles` impl.
borrowed_ref_arg!(
    [u8],
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

/// Decoded into a local: the wire form is a length prefix and elements,
/// which the in-memory vector does not share.
impl<'r, T, const N: usize> RefArg<'r> for heapless::Vec<T, N>
where
    heapless::Vec<T, N>: ReadXDR<'r> + ReceiveAuthHandles,
{
    type Wire = Self;

    #[inline(always)]
    fn get(wire: &Self::Wire) -> &Self {
        wire
    }
}

/// Decoded into a local: the wire form is a length prefix and elements,
/// which the in-memory vector does not share.
#[cfg(feature = "alloc")]
impl<'r, T, const MAX: u32> RefArg<'r> for xdrs::VecM<T, MAX>
where
    xdrs::VecM<T, MAX>: ReadXDR<'r> + ReceiveAuthHandles,
{
    type Wire = Self;

    #[inline(always)]
    fn get(wire: &Self::Wire) -> &Self {
        wire
    }
}
