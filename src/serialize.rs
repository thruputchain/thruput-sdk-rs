pub use xdrs::{WriteXDR, WriteResult, write_xdr, ReadXDR, ReadResult, read_xdr, ReadXDROwned};
pub use xdrs::serialize_txs::decode as decode_xdr;

#[cfg(feature = "alloc")]
mod implementation {
    pub use xdrs::serialize_txs::encode as encode_xdr;
}

#[cfg(not(feature = "alloc"))]
mod implementation {
    use super::*;
    use crate::static_mu_buf;

    const ENCODE_MAX: usize = 500;
    static_mu_buf! {
        fn get_encode_buf<ENCODE_MAX>(size_hint: Option<usize>) -> EncodeBuf;
    }

    pub fn encode_xdr<'a, T>(x: &'a T) -> WriteResult<EncodeBuf>
    where
        T: WriteXDR + ?Sized
    {
        let mut buf = get_encode_buf(None);
        let written = xdrs::serialize_txs::encode_into(x, buf.spare_capacity_mut())?;
        unsafe { buf.set_len(written); }

        Ok(buf)
    }
}

pub use implementation::*;
