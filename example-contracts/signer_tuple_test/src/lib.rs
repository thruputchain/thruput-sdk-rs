#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::signer::Signed;
use sdk::*;

/// A tuple struct contracttype with a Signed field.
#[contracttype]
pub struct TupleAuth(pub u64, pub Signed<u32>);

#[derive(Default)]
pub struct SignerTupleTest;

#[contract(storage = false)]
impl SignerTupleTest {
    pub fn consume_tuple(auth: Signed<TupleAuth>) -> u64 {
        let inner: TupleAuth = auth.consume().0;
        inner.0
    }
}
