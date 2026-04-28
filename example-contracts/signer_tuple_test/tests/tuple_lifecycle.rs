//! Integration test for tuple struct contracttype with Signer fields.

#![allow(unexpected_cfgs)]

use sdk::collect_auth_handles::CollectAuthHandles;
use sdk::mintable::{SignerPreimage, build_auth_token_node};
use sdk::receive_auth_handles::ReceiveAuthHandles;
use sdk::signer::Signer;
use sdk::*;
use signer_tuple_test::TupleAuth;
use signer_tuple_test::__tupleauth_preimage;

fn addr(byte: u8) -> ContractAddress {
    ContractAddress::from([byte; 32])
}

#[test]
fn tuple_struct_auth_token_tree() {
    let addr_leaf = addr(0xAA);
    let addr_consumer = addr(0xBB);

    let preimage = SignerPreimage {
        consumer: addr_consumer.clone(),
        data: __tupleauth_preimage::Preimage(
            42u64,
            SignerPreimage {
                consumer: addr_leaf.clone(),
                data: 99u32,
            },
        ),
    };

    let mut root_node = build_auth_token_node::<TupleAuth>(preimage.data);
    root_node.consumer_address = preimage.consumer;

    assert_eq!(root_node.consumer_address, addr_consumer);
    // The nested Signer<u32> produces one child node
    assert_eq!(root_node.child_nodes.len(), 1);
    assert_eq!(root_node.child_nodes[0].consumer_address, addr_leaf);
}

#[test]
fn tuple_struct_payload_roundtrip() {
    let addr_leaf = addr(0xAA);
    let addr_consumer = addr(0xBB);

    let preimage = SignerPreimage {
        consumer: addr_consumer,
        data: __tupleauth_preimage::Preimage(
            42u64,
            SignerPreimage {
                consumer: addr_leaf,
                data: 99u32,
            },
        ),
    };

    let mut root_node = build_auth_token_node::<TupleAuth>(preimage.data);
    root_node.consumer_address = preimage.consumer;

    let payload_bytes: &[u8] = root_node.payload.as_ref();
    let mut decoded: TupleAuth =
        sdk::cxs::decode(payload_bytes).expect("payload decode failed");

    // Before ReceiveAuthHandles: Signer has sentinel handle
    assert_eq!(decoded.1.handle(), u32::MAX);

    // Fill in real handle
    let child_handles = vec![10u32];
    let mut iter = child_handles.into_iter();
    decoded.receive_auth_handles(&mut iter);

    assert_eq!(decoded.0, 42);
    assert_eq!(decoded.1.handle(), 10);
}

#[test]
fn tuple_struct_collect_handles() {
    let mut auth = TupleAuth(42, Signer::default());
    let mut iter = [5u32].into_iter();
    auth.receive_auth_handles(&mut iter);

    let mut buf = sdk::collect_auth_handles::AuthHandleBuf::new();
    auth.collect_auth_handles(&mut buf);
    assert_eq!(buf.as_slice(), &[5]);
}
