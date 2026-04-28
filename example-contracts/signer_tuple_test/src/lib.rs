#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::signer::Signer;
use sdk::*;

/// A tuple struct contracttype with a Signer field.
#[contracttype]
pub struct TupleAuth(pub u64, pub Signer<u32>);

#[derive(Default)]
pub struct SignerTupleTest;

#[contract(storage = false)]
impl SignerTupleTest {
    pub fn consume_tuple(auth: Signer<TupleAuth>) -> u64 {
        let inner: TupleAuth = auth.consume();
        inner.0
    }
}
