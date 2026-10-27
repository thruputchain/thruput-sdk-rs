// The rest of this vocabulary reaches the crate root through `xdr`.
pub use xdrs::{ReadXDROwned, WriteResult, WriteXDR, read_xdr, write_xdr};

#[cfg(feature = "alloc")]
pub fn encode_xdr<T>(x: &T) -> WriteResult<alloc::vec::Vec<u8>>
where
    T: WriteXDR + ?Sized,
{
    xdrs::cxs::encode(x)
}

pub fn encode_xdr_into<T>(x: &T, buf: &mut [u8]) -> WriteResult<usize>
where
    T: WriteXDR + ?Sized,
{
    // SAFETY: `MU<u8>` matches the layout of `u8`; the allocation is unchanged in size.
    let buf_mu = unsafe {
        core::slice::from_raw_parts_mut(
            buf.as_mut_ptr().cast::<core::mem::MaybeUninit<u8>>(),
            buf.len(),
        )
    };
    xdrs::cxs::encode_into(x, buf_mu)
}
