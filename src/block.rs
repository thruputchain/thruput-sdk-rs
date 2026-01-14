use crate::exn::*;
use crate::sys::raw_GetBlockHeight;
use groundhog_xdr::types::BlockHeight;

pub fn get_block_height() -> BlockHeight {
    let mut block: BlockHeight = 0i64;
    raw_GetBlockHeight(&mut block).unwrap_or_else(|errno| {
        let info = SyscallExn::new(
            crate::sys::SyscallNo::kGetBlockHeight,
            errno,
            #[cfg(feature = "debug")]
            "GetBlockHeight",
        );
        crate::abort(&info);
    });
    block
}
