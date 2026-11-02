//! RV32 end-to-end check of `ref_args`'s by-reference methods: this contract
//! calls each one on a deployed `ref_args`, so the guest decodes every kind
//! of `&T` parameter from real calldata, and checks that the borrowed ones
//! point into it.
//!
//! A failed check traps (contracts build with `panic=immediate-abort`), so
//! `tests/rv32.rs` reports only that the call faulted. `tests/native.rs`
//! runs the same checks with both contracts as host mocks, and
//! `ref_args`'s own tests name the values they compare.

#![cfg_attr(not(feature = "dev"), no_std)]

extern crate alloc;

thruput_sdk::declare_mock_contract!();

use ref_args::{Bytes, Label, Point, RefArgsClient, Words, expected};
use thruput_sdk::xdr::{BytesM, VecM};
use thruput_sdk::*;

pub struct RefArgsCaller;

#[contract(client = RefArgsCallerClient)]
impl RefArgsCaller {
    /// Call every by-reference method of the `ref_args` contract at
    /// `target`, trapping on any wrong result.
    pub fn run(target: &ContractAddress) {
        let client = RefArgsClient::new(target);

        // Bounded vectors, alone and mixed with owned and borrowed params.
        let bytes = Bytes::from_slice(&[1, 2, 3, 250])
            .unwrap_or_else(|_| panic!("bytes over capacity"));
        let words: Words = VecM(alloc::vec![10, 20, 30]);
        assert_eq!(client.sum_bytes(&bytes), 256);
        assert_eq!(client.sum_words(&words), 60);
        let owner = ContractAddress::new([7; 32]);
        assert_eq!(client.mixed(100, &owner, &bytes, &words, &[9u8; 5]), 119);

        // Borrowed parameters point into the guest's calldata buffer.
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

        // Values decoded into locals.
        assert_eq!(
            client.scalars(&7, &-3, &true, &(1u128 << 64)),
            5 + (1 << 64)
        );
        assert_eq!(
            client.composites(
                &Some(4),
                &(6, ContractAddress::new([8; 32])),
                &[1, 2, 3],
                &[[9; 8], [10; 8]],
                &[
                    ContractAddress::new([2; 32]),
                    ContractAddress::new([5; 32])
                ],
            ),
            50
        );
        let label: Label = "hello"
            .try_into()
            .unwrap_or_else(|_| panic!("label over capacity"));
        let blob: BytesM<16> = alloc::vec![1u8, 2, 3]
            .try_into()
            .unwrap_or_else(|_| panic!("blob over capacity"));
        assert_eq!(client.text_lengths(&label, &blob), 8);
        let more = [Point { x: 3, y: 4 }, Point { x: 5, y: 6 }];
        assert_eq!(client.points(&Point { x: 1, y: 2 }, &more), 21);

        // Slices decoded into local vectors.
        let owners =
            [ContractAddress::new([3; 32]), ContractAddress::new([4; 32])];
        assert_eq!(client.slices(&[10, 20, 30], &owners), 67);

        // A vector of byte strings borrowed from calldata.
        let first = [1u8; 5];
        let second = [2u8; 3];
        let list: VecM<&[u8], 4> = VecM(alloc::vec![&first[..], &second[..]]);
        let more_list: VecM<&[u8], 4> = VecM(alloc::vec![&second[..]]);
        let (total, gap) = client.byte_string_lists(list.clone(), &more_list);
        assert_eq!(total, 11);
        let encoded =
            encode_xdr(&list).unwrap_or_else(|_| panic!("list encodes"));
        let at = |needle: &[u8]| {
            encoded
                .windows(needle.len())
                .position(|w| w == needle)
                .unwrap_or_else(|| panic!("marker missing"))
        };
        assert_eq!(gap as usize, at(&second) - at(&first));
    }
}
