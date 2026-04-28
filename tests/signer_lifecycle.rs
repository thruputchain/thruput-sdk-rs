//! Integration test for the full signer lifecycle:
//! - Tree construction via Mintable
//! - Payload encoding/decoding roundtrip
//! - CollectAuthHandles / ReceiveAuthHandles
//! - Nested signers (single + Vec)
//!
//! Scenario: Contract A mints a token destined for B, which contains
//! a nested token for C and a Vec of tokens for C.
//! B consumes its node and gets child Signers. B forwards one to C.

#![allow(unexpected_cfgs)]

use sdk::collect_auth_handles::CollectAuthHandles;
use sdk::mintable::{SignerPreimage, build_auth_token_node};
use sdk::receive_auth_handles::ReceiveAuthHandles;
use sdk::signer::Signer;
use sdk::signer_mapper::MintableType;
use sdk::*;

fn build_signer_node<T: sdk::Mintable>(
    preimage: SignerPreimage<<T as MintableType>::Preimage>,
) -> sdk::AuthTokenNode
where
    T::Preimage: sdk::WriteXDR + for<'r> sdk::ReadXDR<'r>,
    SignerPreimage<T::Preimage>: sdk::WriteXDR + for<'r> sdk::ReadXDR<'r>,
{
    let mut node = build_auth_token_node::<T>(preimage.data);
    node.consumer_address = preimage.consumer;
    node
}

// ── Contract C's auth data (leaf, no nested signers) ──

#[contracttype]
pub struct LeafAuth {
    pub data: u32,
}

// ── Contract B's auth data (has a single Signer<LeafAuth> + Vec<Signer<LeafAuth>>) ──

#[contracttype]
pub struct MiddleAuth {
    pub label: u64,
    pub single_child: Signer<LeafAuth>,
    pub multi_children: Vec<Signer<LeafAuth>>,
}

type MiddleAuthPreimage = <MiddleAuth as MintableType>::Preimage;
type LeafAuthPreimage = <LeafAuth as MintableType>::Preimage;

// ── Helper addresses ──

fn addr(byte: u8) -> ContractAddress {
    ContractAddress::from([byte; 32])
}

// ── Test: build the full AuthTokenNode tree ──

#[test]
fn build_nested_auth_token_tree() {
    let addr_b = addr(0xBB);
    let addr_c1 = addr(0xC1);
    let addr_c2 = addr(0xC2);
    let addr_c3 = addr(0xC3);

    let preimage = SignerPreimage {
        consumer: addr_b.clone(),
        data: MiddleAuthPreimage {
            label: 42,
            single_child: SignerPreimage {
                consumer: addr_c1.clone(),
                data: LeafAuthPreimage { data: 100 },
            },
            multi_children: vec![
                SignerPreimage {
                    consumer: addr_c2.clone(),
                    data: LeafAuthPreimage { data: 200 },
                },
                SignerPreimage {
                    consumer: addr_c3.clone(),
                    data: LeafAuthPreimage { data: 300 },
                },
            ],
        },
    };

    let root_node = build_signer_node::<MiddleAuth>(preimage);

    assert_eq!(root_node.consumer_address, addr_b);
    assert_eq!(root_node.child_nodes.len(), 3);
    assert_eq!(root_node.child_nodes[0].consumer_address, addr_c1);
    assert!(root_node.child_nodes[0].child_nodes.is_empty());
    assert_eq!(root_node.child_nodes[1].consumer_address, addr_c2);
    assert!(root_node.child_nodes[1].child_nodes.is_empty());
    assert_eq!(root_node.child_nodes[2].consumer_address, addr_c3);
    assert!(root_node.child_nodes[2].child_nodes.is_empty());
}

// ── Test: payload roundtrip (encode via Mintable, decode via ReadXDR + ReceiveAuthHandles) ──

#[test]
fn payload_roundtrip_with_receive_handles() {
    let addr_b = addr(0xBB);
    let addr_c1 = addr(0xC1);
    let addr_c2 = addr(0xC2);
    let addr_c3 = addr(0xC3);

    let preimage = SignerPreimage {
        consumer: addr_b,
        data: MiddleAuthPreimage {
            label: 42,
            single_child: SignerPreimage {
                consumer: addr_c1,
                data: LeafAuthPreimage { data: 100 },
            },
            multi_children: vec![
                SignerPreimage {
                    consumer: addr_c2,
                    data: LeafAuthPreimage { data: 200 },
                },
                SignerPreimage {
                    consumer: addr_c3,
                    data: LeafAuthPreimage { data: 300 },
                },
            ],
        },
    };

    let root_node = build_signer_node::<MiddleAuth>(preimage);

    let payload_bytes: &[u8] = root_node.payload.as_ref();
    let mut decoded: MiddleAuth =
        xdrs::cxs::decode(payload_bytes).expect("payload decode failed");

    assert_eq!(decoded.single_child.handle(), u32::MAX);
    for s in &decoded.multi_children {
        assert_eq!(s.handle(), u32::MAX);
    }

    let child_handles = vec![10u32, 20u32, 30u32];
    let mut iter = child_handles.into_iter();
    decoded.receive_auth_handles(&mut iter);

    assert_eq!(decoded.label, 42);
    assert_eq!(decoded.single_child.handle(), 10);
    assert_eq!(decoded.multi_children.len(), 2);
    assert_eq!(decoded.multi_children[0].handle(), 20);
    assert_eq!(decoded.multi_children[1].handle(), 30);
}

// ── Test: CollectAuthHandles extracts handles for forwarding ──

#[test]
fn collect_handles_for_forwarding() {
    let mut middle = MiddleAuth {
        label: 42,
        single_child: Signer::default(),
        multi_children: vec![Signer::default(), Signer::default()],
    };
    let mut iter = [10u32, 20u32, 30u32].into_iter();
    middle.receive_auth_handles(&mut iter);

    let mut buf = sdk::collect_auth_handles::AuthHandleBuf::new();
    middle.collect_auth_handles(&mut buf);
    assert_eq!(buf.as_slice(), &[10, 20, 30]);

    let call_args = (42u64, middle.single_child);
    let mut buf2 = sdk::collect_auth_handles::AuthHandleBuf::new();
    call_args.collect_auth_handles(&mut buf2);
    assert_eq!(buf2.as_slice(), &[10]);
}

// ── Test: leaf payload roundtrip ──

#[test]
fn leaf_payload_roundtrip() {
    let leaf_preimage = SignerPreimage {
        consumer: addr(0xCC),
        data: LeafAuthPreimage { data: 99 },
    };

    let node = build_signer_node::<LeafAuth>(leaf_preimage);

    assert_eq!(node.consumer_address, addr(0xCC));
    assert!(node.child_nodes.is_empty());

    let decoded: LeafAuth = xdrs::cxs::decode(node.payload.as_ref())
        .expect("leaf payload decode failed");
    assert_eq!(decoded.data, 99);
}

// ── Test: full encode_auth_token_node produces valid XDR ──

#[test]
fn encode_auth_token_node_roundtrip() {
    let preimage = SignerPreimage {
        consumer: addr(0xBB),
        data: LeafAuthPreimage { data: 77 },
    };

    let node = build_signer_node::<LeafAuth>(preimage);

    let encoded = sdk::mintable::encode_auth_token_node(&node);
    assert!(!encoded.is_empty());

    let decoded: sdk::AuthTokenNode =
        xdrs::txs::decode(&encoded).expect("AuthTokenNode decode failed");
    assert_eq!(decoded.consumer_address, addr(0xBB));
    assert!(decoded.child_nodes.is_empty());
}
