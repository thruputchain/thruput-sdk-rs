#![doc(hidden)]

pub use cross_call::invoke;
pub use handle::handle_call;

mod handle {
    use crate::invoke::{contract_return, get_calldata};
    use crate::serialize::{self, DeserializeOwned, Serialize};
    use crate::syscall::{SysErr, SyscallErrorCode};

    // Run a handler function on the invocation arguments
    // available in serialized form via `GetCalldata`,
    // and serialize the result for passing to `Return`.
    pub fn handle_call<F, ARGS, RES>(f: F) -> SysErr
    where
        F: FnOnce(ARGS) -> RES,
        ARGS: DeserializeOwned,
        RES: Serialize,
    {
        let call_data = match get_calldata() {
            Ok(x) => x,
            Err(e) => return e, // == SyscallErrorCode::kCalldataParseError
        };
        let args: ARGS = match serialize::from_bytes(&call_data) {
            Ok(args) => args,
            Err(_) => return SyscallErrorCode::kCalldataParseError.into(),
        };
        let result: RES = f(args);
        let result_bytes = match serialize::to_bytes(&result) {
            Ok(bytes) => bytes,
            Err(_) => return SyscallErrorCode::kCalldataParseError.into(),
        };
        contract_return(result_bytes.as_slice());
    }
}

mod cross_call {
    use crate::serialize::{self, DeserializeOwned, Serialize};
    use crate::sys;
    use crate::types::{
        ContractAddress, /* InvocationFlagSet, InvocationFlags, */
        RuntimeMemoryLength,
    };
    use alloc::{vec, vec::Vec};

    pub fn invoke<ARGS, RET>(
        address: &ContractAddress,
        method: u32,
        gas_limit: u64,
        args: &ARGS,
    ) -> RET
    where
        ARGS: Serialize,
        RET: DeserializeOwned,
    {
        // Encode the args
        let call_data =
            serialize::to_bytes(args).expect("Couldn't serialize call data");

        // Invoke the method
        let return_buf =
            invoke_inner(address, method, gas_limit, call_data.as_slice());

        // Deserialize the buffer into a return value ...
        let return_buf = &return_buf.as_slice();
        #[allow(clippy::let_and_return)]
        let return_value = serialize::from_bytes(return_buf)
            .expect("Couldn't deserialize return value");

        return_value
    }

    fn invoke_inner(
        address: &ContractAddress,
        method: u32,
        gas_limit: u64,
        call_data: &[u8],
    ) -> Vec<u8> {
        // Perform the cross-contract call

        let mut return_len = 0;
        sys::raw_Invoke(
            address,
            method,
            gas_limit,
            call_data,
            &mut return_len,
        )
        .expect("Contract invocation failed");

        let mut return_buf = vec![0; return_len as usize];
        let mut written: RuntimeMemoryLength = 0;
        sys::raw_GetReturnedBuffer(return_buf.as_mut_slice(), &mut written)
            .expect("Couldn't get returned buffer");

        if written != return_len {
            panic!("Unexpected return buffer length");
        }

        return_buf
    }
}
