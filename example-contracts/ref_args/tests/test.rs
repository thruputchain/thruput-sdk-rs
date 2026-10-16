extern crate alloc;

use ref_args::{Bytes, Label, Point, RefArgsClient, Words, expected};
use thruput_sdk::xdr::{BytesM, VecM};
use thruput_sdk::*;

hash_ident! { static NONCE: SHA256Hash; }
hash_ident! { static CALLER: ContractAddress; }

/// Deploys the contract and runs `body` against a client for it.
fn with_client(
    body: impl FnOnce(RefArgsClient) + std::panic::UnwindSafe + 'static,
) {
    let env = TestEnvironment::no_cost();
    let hash = *ref_args::register_mock_contract(&env).as_hash();

    let result = env.transact(|tx| {
        tx.call(&CALLER).run(move || {
            let addr = deploy_contract(&NONCE, &hash);
            body(RefArgsClient::new(&addr));
        })
    });
    result.assert_success();
}

/// Bounded vectors taken by reference reach the method with the values the
/// caller sent, alone and mixed with owned and borrowed parameters.
#[test]
fn bounded_vectors_by_reference() {
    with_client(|client| {
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
    });
}

/// Borrowed parameters point into the calldata buffer: each one sits,
/// relative to the first, exactly where its bytes are in the encoded
/// arguments. A copy anywhere would put that argument somewhere else.
#[test]
fn borrowed_parameters_point_into_calldata() {
    with_client(|client| {
        let offsets = client.borrowed_offsets(
            &expected::ADDRESS,
            expected::BYTES,
            expected::TEXT,
            &expected::FIXED,
        );
        assert_eq!(offsets, expected::borrowed_offsets());
        let strings: &[&[u8]] = &[expected::STRING_0, expected::STRING_1];
        let offsets = client.borrowed_list_offsets(
            &expected::HASH,
            strings,
            &[expected::WORD],
        );
        assert_eq!(offsets, expected::borrowed_list_offsets());
    });
}

/// Scalars, `Option`, tuples, and arrays by reference decode into locals
/// with the sent values.
#[test]
fn owned_values_by_reference() {
    with_client(|client| {
        assert_eq!(
            client.scalars(&7, &-3, &true, &(1u128 << 64)),
            5 + (1 << 64)
        );

        let owner_a = ContractAddress::new([2; 32]);
        let owner_b = ContractAddress::new([5; 32]);
        // 4 + 6 + 8 (pair's address) + (1 + 2 + 3) + (9 + 10) + (2 + 5)
        assert_eq!(
            client.composites(
                &Some(4),
                &(6, ContractAddress::new([8; 32])),
                &[1, 2, 3],
                &[[9; 8], [10; 8]],
                &[owner_a, owner_b],
            ),
            50
        );
        assert_eq!(
            client.composites(
                &None,
                &(0, ContractAddress::new([0; 32])),
                &[0; 3],
                &[[0; 8]; 2],
                &[ContractAddress::new([0; 32]); 2],
            ),
            0
        );
    });
}

/// Bounded strings and `#[contract_type]` values by reference.
#[test]
fn strings_and_contract_types_by_reference() {
    with_client(|client| {
        let label: Label = "hello".try_into().unwrap();
        let blob: BytesM<16> = alloc::vec![1u8, 2, 3].try_into().unwrap();
        assert_eq!(client.text_lengths(&label, &blob), 8);

        let point = Point { x: 1, y: 2 };
        let more = [Point { x: 3, y: 4 }, Point { x: 5, y: 6 }];
        assert_eq!(client.points(&point, &more), 21);
        assert_eq!(client.points(&point, &[]), 3);
    });
}

/// Slices of non-byte elements decode into local vectors.
#[test]
fn slices_by_reference() {
    with_client(|client| {
        let owners =
            [ContractAddress::new([3; 32]), ContractAddress::new([4; 32])];
        assert_eq!(client.slices(&[10, 20, 30], &owners), 67);
        assert_eq!(client.slices(&[], &[]), 0);
    });
}

/// A `VecM` of byte strings borrows each string from calldata, by value
/// and by reference: the second string sits right after the first one's
/// bytes and its own length prefix.
#[test]
fn vectors_of_borrowed_byte_strings() {
    with_client(|client| {
        let first = [1u8; 5];
        let second = [2u8; 3];
        let strings: VecM<&[u8], 4> =
            VecM(alloc::vec![&first[..], &second[..]]);
        let more: VecM<&[u8], 4> = VecM(alloc::vec![&second[..]]);
        let (total, gap) = client.byte_string_lists(strings.clone(), &more);
        assert_eq!(total, 11);

        let encoded = thruput_sdk::encode_xdr(&strings).unwrap();
        let first_at = encoded.windows(5).position(|w| w == first).unwrap();
        let second_at = encoded.windows(3).position(|w| w == second).unwrap();
        assert_eq!(gap as usize, second_at - first_at);
    });
}
