#![allow(clippy::result_large_err)]

use sdk::*;
use swap::*;

#[derive(ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
pub struct Cfg {
    pub swap_pair: Address,
}

impl Cfg {
    pub fn pair(&self) -> ISwapV2PairClient {
        ISwapV2PairClient::new(&self.swap_pair).with_commit_locking()
    }
}
