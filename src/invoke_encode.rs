//! Encoding support for invoke

// Provide `decode` directly from `xdrs`
pub use xdrs::serialize_txs::decode;

use crate::invoke_error::*;
use xdrs::WriteXDR;

#[cfg(any(feature = "dynamic_bufs", feature = "mock"))]
mod implementation {
    // Use dynamically-allocated bufs

    use super::*;
    use alloc::vec::Vec;
    use xdrs::serialize_txs::encode;

    pub const RETURN_DATA_MAX: Option<usize> = None;

    pub type CallData = Vec<u8>;
    pub type ReturnData = Vec<u8>;

    pub fn get_return_buffer(size_hint: Option<usize>) -> CallData {
        Vec::with_capacity(size_hint.unwrap())
    }

    pub fn encode_into_calldata<ARGS>(args: &ARGS) -> InvokeResult<CallData>
    where
        ARGS: WriteXDR,
    {
        encode(args).map_err(|_| InvokeError::EncodeArgs)
    }
}

#[cfg(not(any(feature = "dynamic_bufs", feature = "mock")))]
mod implementation {
    // Sequentially-share one statically-allocated buf for encodings of invocation args and return vals

    use super::*;
    use crate::contract_panic;
    use crate::static_mu_buf;
    use xdrs::serialize_txs::encode_into;
    use xdrs::WriteError;

    const INVOKE_MAX: usize = 500;

    static_mu_buf! {
        pub fn get_invoke_buffer<INVOKE_MAX>(size_hint: Option<usize>) -> InvokeData;
    }

    pub type CallData = InvokeData;
    pub type ReturnData = InvokeData;

    // Use the shared invoke buffer as the return buffer
    pub const RETURN_DATA_MAX: Option<usize> = Some(INVOKE_MAX);
    pub use get_invoke_buffer as get_return_buffer;

    pub fn encode_into_calldata<ARGS>(args: &ARGS) -> InvokeResult<CallData>
    where
        ARGS: WriteXDR,
    {
        // Use the shared invoke buffer as the calldata buffer
        const CALL_DATA_MAX: Option<usize> = Some(INVOKE_MAX);
        use get_invoke_buffer as get_calldata_buffer;

        let mut call_data = get_calldata_buffer(None);

        let len = encode_into(args, call_data.spare_capacity_mut())
            .inspect_err(|e| {
                if matches!(e, WriteError::SpaceExhausted) {
                    if let Some(max) = CALL_DATA_MAX {
                        contract_panic!(
                            "overflowed calldata buffer, which has fixed size {}",
                            max
                        );
                    }
                }
            })
            .map_err(|_| InvokeError::EncodeArgs)?;

        // SAFETY: The serializer has reported this many bytes written
        unsafe {
            call_data.set_len(len);
        }

        Ok(call_data)
    }
}

pub use implementation::*;
