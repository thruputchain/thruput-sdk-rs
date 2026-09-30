//! RV32 end-to-end check of the by-reference methods: the tester deploys
//! the plain build and calls each one, so the guest decodes `&Bytes`,
//! `&Words`, `&ContractAddress`, and `&[u8]` from real calldata.

use crate::{Bytes, RefArgsClient, Words};
pub use bottle_test_if::*;
use sdk::storage::Value;
use sdk::xdr::VecM;
use sdk::*;

#[derive(Debug)]
#[storage]
pub struct BT {
    target_addr: Value<Option<ContractAddress>>,
}

#[contract(exports = "bottle_exports")]
impl BottleTest for BT {
    fn setup(&mut self, target_contract_hash: &SHA256Hash) {
        let nonce: SHA256Hash = [7; 32].into();
        let target_addr = deploy_contract(&nonce, target_contract_hash);

        self.target_addr.set(&Some(target_addr));
    }

    fn run(&mut self) {
        let target_addr = self.target_addr.get().unwrap_or_else(|| {
            panic!(
                "bottle-test: target_addr unset; please run setup() before run()"
            )
        });
        let client = RefArgsClient::new(&target_addr);

        let bytes = Bytes::from_slice(&[1, 2, 3, 250])
            .unwrap_or_else(|_| panic!("bottle-test: bytes over capacity"));
        let words: Words = VecM(alloc::vec![10, 20, 30]);
        assert_eq!(client.sum_bytes(&bytes), 256);
        assert_eq!(client.sum_words(&words), 60);
        assert_eq!(client.sum_bytes(&Bytes::new()), 0);
        assert_eq!(client.sum_words(&VecM(alloc::vec![])), 0);

        let owner = ContractAddress::new([7; 32]);
        let raw = [9u8; 5];
        // 100 + 7 + (4 + 3 + 5)
        assert_eq!(client.mixed(100, &owner, &bytes, &words, &raw), 119);
    }
}
