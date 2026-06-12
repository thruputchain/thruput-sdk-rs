#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::signer::Signed;
use sdk::*;

/// A tuple struct contract_type with a Signed field.
#[contract_type]
pub struct TupleAuth(pub u64, pub Signed<u32>);

pub struct SignerTupleTest;

#[contract]
impl SignerTupleTest {
    pub fn consume_tuple(auth: Signed<TupleAuth>) -> u64 {
        let inner: TupleAuth = auth.consume().0;
        inner.0
    }
}
