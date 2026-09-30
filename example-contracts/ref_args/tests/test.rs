extern crate alloc;

use ref_args::{Bytes, RefArgsClient, Words};
use sdk::xdr::VecM;
use sdk::*;

hash_ident! { static NONCE: SHA256Hash; }
hash_ident! { static CALLER: ContractAddress; }

/// Every by-reference parameter shape reaches the method with the value the
/// caller sent: bounded vectors decoded into a local, and fixed-size types
/// borrowed from the calldata, alone and mixed with an owned parameter.
#[test]
fn reference_parameters_decode_to_the_sent_values() {
    let env = TestEnvironment::no_cost();
    let hash = *ref_args::register_mock_contract(&env).as_hash();

    let result = env.transact(|tx| {
        tx.call(&CALLER).run(move || {
            let addr = deploy_contract(&NONCE, &hash);
            let client = RefArgsClient::new(&addr);

            let bytes = Bytes::from_slice(&[1, 2, 3, 250]).unwrap();
            let words: Words = VecM(alloc::vec![10, 20, 30]);
            assert_eq!(client.sum_bytes(&bytes), 256);
            assert_eq!(client.sum_words(&words), 60);
            assert_eq!(client.sum_bytes(&Bytes::new()), 0);
            assert_eq!(client.sum_words(&VecM(alloc::vec![])), 0);

            let owner = ContractAddress::new([7; 32]);
            let raw = [9u8; 5];
            // 100 + 7 + (4 + 3 + 5)
            assert_eq!(client.mixed(100, &owner, &bytes, &words, &raw), 119);
        })
    });
    result.assert_success();
}
