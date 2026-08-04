//! Tests for `Signed::consume` against hand-built token nodes.

#![allow(unexpected_cfgs)]

use sdk::mintable::{
    AuthTokenPreimage, MintableType, build_auth_token_node,
    encode_auth_token_node,
};
use sdk::signer::Signed;
use sdk::*;

const TEST_ADDR: ContractAddress = ContractAddress::new([1u8; 32]);

#[contract_type]
pub struct LeafAuth {
    pub data: u32,
}

type LeafAuthPreimage = <LeafAuth as MintableType>::Preimage;

#[test]
fn consume_aborts_on_leftover_child_handles() {
    let env = TestEnvironment::no_cost();
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            // LeafAuth has no Signed fields, but hand the minted node an
            // extra child token anyway. consume() must reject the token
            // rather than silently drop the child handle.
            let mut node =
                build_auth_token_node::<LeafAuth>(AuthTokenPreimage {
                    consumer: sdk::get_current_address(),
                    data: LeafAuthPreimage { data: 7 },
                });

            let extra_child =
                build_auth_token_node::<u64>(AuthTokenPreimage {
                    consumer: sdk::get_current_address(),
                    data: 0u64,
                });
            node.child_nodes = vec![extra_child].try_into().unwrap();

            let encoded = encode_auth_token_node(&node);
            let expiry = sdk::block::get_block_height();
            let token = sdk::signer::mint_opaque_auth_token(expiry, &encoded);
            let typed: Signed<LeafAuth> = token.assume();
            let _ = typed.consume();
        })
    });
    result.assert_abort();
}

#[test]
fn assume_tokens_mismatch_throws_cross_call_decode_error() {
    use sdk::exn::{CallingError, LibraryExn, RawException};
    use sdk::signer::{AssumeTokens, Opaque};

    let env = TestEnvironment::no_cost();
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            // A called wallet decides how many tokens it returns; the
            // wrong count must surface as the same catchable cross-call
            // error as a return-payload decode failure, not a panic.
            let tokens: Vec<Signed<Opaque>> = vec![];
            let (_,): (Signed<u64>,) =
                AssumeTokens::try_assume_tokens(tokens).or_re_throw();
        })
    });
    result.assert_abort();

    let caught =
        RawException::from_abort_buf(result.frame().abort_buffer()).unwrap();
    let exn = caught
        .try_as::<LibraryExn>()
        .expect("abort payload decodes as LibraryExn");
    assert!(
        matches!(exn, LibraryExn::Calling(CallingError::DecodingSuccess(_))),
        "expected Calling(DecodingSuccess), got {exn:?}"
    );
}

#[test]
fn consume_succeeds_with_exact_child_handles() {
    let env = TestEnvironment::no_cost();
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            let node = build_auth_token_node::<LeafAuth>(AuthTokenPreimage {
                consumer: sdk::get_current_address(),
                data: LeafAuthPreimage { data: 7 },
            });

            let encoded = encode_auth_token_node(&node);
            let expiry = sdk::block::get_block_height();
            let token = sdk::signer::mint_opaque_auth_token(expiry, &encoded);
            let typed: Signed<LeafAuth> = token.assume();
            let (leaf, minter) = typed.consume();
            assert_eq!(leaf.data, 7);
            assert_eq!(minter, sdk::get_current_address());
        })
    });
    result.assert_success();
}
