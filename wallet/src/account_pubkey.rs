use groundhog_xdr::types::*;
use xdrs::WriteXDR;

#[derive(WriteXDR)]
pub struct PubkeyAccountPreimage {
    pub pubkey: PublicKey,
    pub nonce: ReplayNonce,
}
