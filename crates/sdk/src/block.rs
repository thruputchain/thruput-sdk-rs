use crate::sys::raw_GetBlockHeight;
use groundhog_xdr::types::BlockHeight;

pub fn get_block_height() -> BlockHeight {
    let mut block: BlockHeight = 0i64.into();
    raw_GetBlockHeight(&mut block).unwrap();
    block
}
