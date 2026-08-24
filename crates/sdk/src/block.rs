use crate::exn::syscall_throw;
use crate::sys::{self, SyscallNo};
use groundhog_xdr::types::BlockHeight;

/// The height of the block the current transaction executes in.
///
/// Throws a syscall exception (aborting the frame) if the host call fails.
pub fn get_block_height() -> BlockHeight {
    let mut block: BlockHeight = 0i64.into();
    sys::raw_GetBlockHeight(&mut block)
        .unwrap_or_else(|e| syscall_throw(SyscallNo::kGetBlockHeight, e));
    block
}
