//! Serialization
//!
//! # Decoding
//!
//! We provide `decode_xdr` directly from `xdrs`.
//!
//! # Encoding
//!
//! Allocate a buffer on the stack to receive the encoding.

pub use xdrs::cxs::decode as decode_xdr;
pub use xdrs::{
    ReadError, ReadResult, ReadXDR, ReadXDROwned, WriteError, WriteResult,
    WriteXDR, read_xdr, write_xdr,
};

#[cfg(not(feature = "alloc"))]
use crate::internal::mu_buf::MUBuf;

/// The single fixed-buffer size for every no-alloc scratch / return / calldata
/// buffer. It is sized above the protocol's value-channel caps (events,
/// storage bytestrings, and returns are all 1024) with headroom for calldata
/// and cross-call args, which the protocol leaves unbounded. There is no
/// SDK-side per-channel cap: the host enforces the protocol limits, so this is
/// purely "how much a no-alloc contract is willing to buffer."
pub const SCRATCH_MAX: usize = 4096;

/// The SDK's one byte-buffer type: a heap `Vec` under `alloc`, a fixed
/// [`MUBuf`] in the no-alloc guest. Used for calldata, cross-call args,
/// returns/aborts, events, storage values — everything the SDK encodes or
/// receives as bytes.
#[cfg(feature = "alloc")]
pub type ScratchBuf = alloc::vec::Vec<u8>;
#[cfg(not(feature = "alloc"))]
pub type ScratchBuf = MUBuf<SCRATCH_MAX>;

/// A fresh scratch buffer. Under `alloc` it pre-reserves `SCRATCH_MAX` so the
/// reused per-instance scratch never reallocates for in-bounds payloads.
#[cfg(feature = "alloc")]
pub fn new_scratch() -> ScratchBuf {
    alloc::vec::Vec::with_capacity(SCRATCH_MAX)
}
#[cfg(not(feature = "alloc"))]
pub fn new_scratch() -> ScratchBuf {
    MUBuf::new()
}

/// CXS-encode `x` into `buf`, replacing its contents. No SDK-side size cap:
/// under `alloc` the buffer grows organically; in the no-alloc guest the fixed
/// capacity bounds it. The host enforces the protocol's per-channel limits.
pub fn encode_scratch<T>(buf: &mut ScratchBuf, x: &T) -> WriteResult<()>
where
    T: WriteXDR + ?Sized,
{
    #[cfg(feature = "alloc")]
    {
        buf.clear();
        // `BufWriter` grows the `Vec`; `take`/restore keeps its reserved
        // capacity so repeated encodes into the reused scratch don't realloc.
        let mut writer =
            xdrs::cxs::buf_writer::BufWriter::from(core::mem::take(buf));
        let result = x.write_xdr(&mut writer);
        *buf = writer.into();
        result
    }
    #[cfg(not(feature = "alloc"))]
    {
        let written = xdrs::cxs::encode_into(x, buf.spare_capacity_mut())?;
        // Safety: `encode_into` initialized exactly `written` bytes.
        unsafe {
            buf.set_len(written);
        }
        Ok(())
    }
}

/// Owned-buffer convenience for cold paths (hashing, auth-token build, tests):
/// encode into a fresh scratch and return it. Hot paths should encode into a
/// reused scratch via [`encode_scratch`].
pub fn encode_xdr<T>(x: &T) -> WriteResult<ScratchBuf>
where
    T: WriteXDR + ?Sized,
{
    let mut buf = new_scratch();
    encode_scratch(&mut buf, x)?;
    Ok(buf)
}
