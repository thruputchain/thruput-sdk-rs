//! A deprecated utility

use groundhog_xdr::syscalls::SysErr;
use groundhog_xdr::syscall_error::SyscallErrorCode;
use crate::contract_op::{contract_return, get_calldata};

use xdrs::{ReadXDROwned, WriteXDR};
use xdrs::serialize_txs::{encode, decode};

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
        Err(_) => return SyscallErrorCode::kSyscallErrorCodeCalldataParseError.into(),
    };

    let result: RES = f(args);
    let result_bytes = match encode(&result) {
        Ok(r) => r,
        Err(_) => return SyscallErrorCode::kSyscallErrorCodeReturnParseError.into(),
    };

    contract_return(result_bytes.as_slice());
}
