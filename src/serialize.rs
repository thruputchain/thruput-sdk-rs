pub use xdrs::{WriteXDR, WriteResult, write_xdr, ReadXDR, ReadResult, read_xdr, ReadXDROwned};
pub use xdrs::serialize_txs::decode as decode_xdr;
#[cfg(feature = "alloc")]
pub use xdrs::serialize_txs::encode as encode_xdr;

use crate::sysresult::{Error, Result};

#[cfg(feature = "alloc")]
pub fn serialize_xdr<T: WriteXDR + ?Sized>(x: &T) -> Result<alloc::vec::Vec<u8>> {
    xdrs::serialize_txs::encode(x).map_err(|e|
        Error::serialization(e))
}
