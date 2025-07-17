#![doc(hidden)]

// for the error values ignored in `contract_panic!`
#[allow(unused_variables)]

pub use cross_call::invoke;

#[cfg(feature = "alloc")]
pub use handle::handle_call;

#[cfg(feature = "alloc")]
// This is deprecated; it is currently used only by tests
mod handle {
    use crate::invoke::{contract_return, get_calldata};
    use crate::syscall::{SysErr, SyscallErrorCode};

    use ::xdrs::{ReadXDROwned, WriteXDR};
    use ::xdrs::serialize_txs::{encode, decode};

    // Run a handler function on the invocation arguments
    // available in serialized form via `GetCalldata`,
    // and serialize the result for passing to `Return`.
    pub fn handle_call<F, ARGS, RES>(f: F) -> SysErr
    where
        F: FnOnce(ARGS) -> RES,
        ARGS: ReadXDROwned,
        RES: WriteXDR,
    {
        let boxed_call_data = match get_calldata() {
            Ok(x) => x,
            Err(e) => return e, // == SyscallErrorCode::kCalldataParseError
        };
        let call_data: &[u8] = boxed_call_data.as_ref();
        let args: ARGS = match decode(call_data) {
            Ok(args) => args,
            Err(_) => return SyscallErrorCode::kCalldataParseError.into(),
        };

        let result: RES = f(args);
        let result_bytes = match encode(&result) {
            Ok(r) => r,
            Err(_) => return SyscallErrorCode::kReturnParseError.into(),
        };

        contract_return(result_bytes.as_slice());
    }
}

mod cross_call {
    use crate::sys;
    use crate::types::{
        ContractAddress, /* InvocationFlagSet, InvocationFlags, */
        RuntimeMemoryLength,
    };
    use crate::contract_panic;
    use ::groundhog_xdr::contract::ContractEntrypointID;
    use ::xdrs::{ReadXDROwned, WriteXDR};
    use ::xdrs::serialize_txs::{encode_into, decode};
    use core::mem::MaybeUninit as MU;

    pub fn invoke<ARGS, RET>(
        address: &ContractAddress,
        entrypoint: ContractEntrypointID,
        gas_limit: u64,
        args: &ARGS,
    ) -> RET
    where
        ARGS: WriteXDR,
        RET: ReadXDROwned,
    {
        // Encode the args
        const CALLDATA_BUFSZ: usize = 1000;
        let mut calldata_mu: [MU<u8>; CALLDATA_BUFSZ] =
            [const { MU::uninit() }; CALLDATA_BUFSZ];

        let written = encode_into(args, &mut calldata_mu).unwrap_or_else(|e|
            contract_panic!("couldn't serialize call data: {}", e));

        let call_data: &[u8] = unsafe { (&calldata_mu[..written]).assume_init_ref() };

        // Make a buffer to receive the result
        const RETURN_BUFSZ: usize = 1000;
        let mut return_buf: [u8; RETURN_BUFSZ] = [0; RETURN_BUFSZ];

        // Invoke the method
        let written =
            invoke_inner(address, entrypoint, gas_limit, call_data, &mut return_buf);

        let return_buf = &return_buf[..written];

        // Deserialize the buffer into a return value ...
        decode(return_buf).unwrap_or_else(|e|
            contract_panic!("couldn't deserialize return value: {}", e))
    }

    // At least this part won't be monomorphized multiple times
    #[inline(never)]
    fn invoke_inner(
        address: &ContractAddress,
        entrypoint: ContractEntrypointID,
        gas_limit: u64,
        call_data: &[u8],
        return_buf: &mut [u8],
    ) -> usize {
        let mut return_len = 0;
        let entrypoint = u64::from_be_bytes(entrypoint.into_inner());

        // Perform the cross-contract call
        sys::raw_Invoke(
            address,
            entrypoint,
            gas_limit,
            call_data,
            0, // TODO(geoff): invoke flags go here
            &mut return_len,
        ).unwrap_or_else(|_| contract_panic!("contract invocation failed"));

        let return_len = return_len as usize;
        if return_len > return_buf.len() {
            contract_panic!("contract call return data too large for static buffer");
        }

        let mut written: RuntimeMemoryLength = 0;
        sys::raw_GetReturnedBuffer(return_buf, &mut written).unwrap_or_else(|e|
            contract_panic!("couldn't get returned buffer: {}", e));

        if written as usize != return_len {
            contract_panic!("unexpected return buffer length");
        }

        return_len
    }
}
