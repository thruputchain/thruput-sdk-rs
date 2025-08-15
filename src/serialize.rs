pub use xdrs::{WriteXDR, ReadXDR, ReadXDROwned};
pub use xdrs::serialize_txs::decode as decode_xdr;
#[cfg(feature = "alloc")]
pub use xdrs::serialize_txs::encode as encode_xdr;

use crate::sysresult::{Error, Result};

#[cfg(feature = "alloc")]
pub fn serialize_xdr<T: WriteXDR + ?Sized>(x: &T) -> Result<alloc::vec::Vec<u8>> {
    xdrs::serialize_txs::encode(x).map_err(|e|
        Error::serialization(e))
}

// to be removed soon
pub use ::serde::{de::DeserializeOwned, Deserialize, Serialize};
pub use ::bcs::{from_bytes, from_bytes_seed};
#[cfg(feature = "alloc")]
pub use ::bcs::to_bytes;
