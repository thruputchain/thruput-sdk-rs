#![doc(hidden)]

pub use cross_call::invoke;
pub use handle::handle_call;

mod handle {
    use crate::invoke::{contract_return, get_calldata};
    use crate::syscall::{SysErr, SyscallErrorCode};

    use ::xdrs::{ReadXDROwned, WriteXDR};
    use ::xdrs::serialize_txs::{encode, decode};
    use crate::returnable::Returnable;

    // Run a handler function on the invocation arguments
    // available in serialized form via `GetCalldata`,
    // and serialize the result for passing to `Return`.
    pub fn handle_call<F, ARGS, RES>(f: F) -> SysErr
    where
        F: FnOnce(ARGS) -> RES,
        ARGS: ReadXDROwned,
        RES: WriteXDR + Returnable
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

        contract_return(result_bytes.as_slice(), result.force_revert());
    }
}

mod cross_call {
    use crate::sys;
    use crate::types::{
        ContractAddress, /* InvocationFlagSet, InvocationFlags, */
        RuntimeMemoryLength,
    };
    use ::groundhog_xdr::contract::ContractEntrypointID;
    use alloc::{vec, vec::Vec};
    use ::xdrs::{ReadXDROwned, WriteXDR};
    use ::xdrs::serialize_txs::{encode, decode};

    pub fn invoke<ARGS, RET>(
        address: &ContractAddress,
        entrypoint: ContractEntrypointID,
        gas_limit: u64,
        args: &ARGS,
    ) -> Result<RET, crate::syscall::SysErr>
    where
        ARGS: WriteXDR,
        RET: ReadXDROwned,
    {
        // Encode the args
        let call_data = encode(args).or(Err(crate::syscall::SyscallErrorCode::kCalldataParseError))?;

        // Invoke the method
        let return_buf =
            invoke_inner(address, entrypoint, gas_limit, call_data.as_slice())?;

        // Deserialize the buffer into a return value ...
        let return_buf = &return_buf.as_slice();
        #[allow(clippy::let_and_return)]
        let return_value : RET = decode(return_buf)
            .or(Err(crate::syscall::SyscallErrorCode::kReturnParseError))?;

        Ok(return_value)
    }

    // At least this part won't be monomorphized multiple times
    #[inline(never)]
    fn invoke_inner(
        address: &ContractAddress,
        entrypoint: ContractEntrypointID,
        gas_limit: u64,
        call_data: &[u8],
    ) -> Result<Vec<u8>, crate::syscall::SysErr> {
        // Perform the cross-contract call

        let mut return_len = 0;
        let entrypoint = u64::from_be_bytes(entrypoint.into_inner());
        sys::raw_Invoke(
            address,
            entrypoint,
            gas_limit,
            call_data,
            0, // TODO(geoff): invoke flags go here
            &mut return_len,
        )?;

        let mut return_buf = vec![0; return_len as usize];
        let mut written: RuntimeMemoryLength = 0;
        sys::raw_GetReturnedBuffer(return_buf.as_mut_slice(), &mut written)?;

        if written != return_len {
            panic!("Unexpected return buffer length");
        }

        Ok(return_buf)
    }
}
