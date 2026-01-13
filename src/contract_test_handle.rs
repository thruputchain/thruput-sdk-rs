//! A deprecated utility

use crate::contract_op::{contract_return, get_calldata};

use xdrs::serialize_txs::{decode, encode};
use xdrs::{ReadXDROwned, WriteXDR};

// Run a handler function on the invocation arguments
// available in serialized form via `GetCalldata`,
// and serialize the result for passing to `Return`.
pub fn handle_call<F, ARGS, RES>(f: F) -> ()
where
    F: FnOnce(ARGS) -> RES,
    ARGS: ReadXDROwned,
    RES: WriteXDR,
{
    let boxed_call_data = match get_calldata() {
        Ok(x) => x,
        Err(e) => {
            panic!("Failed to get calldata: {:?}", e);
        }
    };
    let call_data: &[u8] = boxed_call_data.as_ref();
    let args: ARGS = match decode(call_data) {
        Ok(args) => args,
        Err(_) => {
            panic!("Failed to decode calldata");
        }
    };

    let result: RES = f(args);
    let result_bytes = match encode(&result) {
        Ok(r) => r,
        Err(_) => {
            panic!("Failed to encode result");
        }
    };

    contract_return(result_bytes.as_slice());
}
