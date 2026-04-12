use sdk::*;

/// Minimal HTLC-like interface whose `create_htlc` method takes a non-Copy
/// `BlockHeight` by value.  When compiled with the `logging` feature active
/// on the codegen crate, the client stub must not move the argument twice.
#[contract(client = Client)]
pub trait HTLC {
    fn initialize(&mut self) -> ();

    fn create_htlc(
        &mut self,
        receiver: &ContractAddress,
        token: &ContractAddress,
        amount: i128,
        expiration_height: BlockHeight,
        hash: &SHA256Hash,
    ) -> ();

    fn claim_htlc(&mut self, hash: &SHA256Hash, preimage: &SHA256Hash) -> ();

    fn refund_htlc(&mut self, hash: &SHA256Hash) -> ();
}
