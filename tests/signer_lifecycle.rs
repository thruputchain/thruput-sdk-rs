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
use sdk::mintable::{Mintable, UnsignedSigner};
use sdk::receive_auth_handles::ReceiveAuthHandles;
use sdk::signer::Signer;
use sdk::*;

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

    // Build unsigned form: A mints for B, which nests C tokens
    let unsigned = UnsignedSigner {
        consumer: addr_b.clone(),
        data: __groundhog_mintable_middle_auth::MiddleAuthUnsigned {
            label: 42,
            single_child: UnsignedSigner {
                consumer: addr_c1.clone(),
                data: __groundhog_mintable_leaf_auth::LeafAuthUnsigned {
                    data: 100,
                },
            },
            multi_children: vec![
                UnsignedSigner {
                    consumer: addr_c2.clone(),
                    data: __groundhog_mintable_leaf_auth::LeafAuthUnsigned {
                        data: 200,
                    },
                },
                UnsignedSigner {
                    consumer: addr_c3.clone(),
                    data: __groundhog_mintable_leaf_auth::LeafAuthUnsigned {
                        data: 300,
                    },
                },
            ],
        },
    };

    let root_node =
        <Signer<MiddleAuth> as Mintable>::build_auth_token_node(unsigned);

    // Root node: consumer = B
    assert_eq!(root_node.consumer_address, addr_b);

    // Root has 3 children: single_child + 2 from multi_children
    assert_eq!(root_node.child_nodes.len(), 3);

    // Child 0: single_child → consumer = C1, leaf (no children)
    assert_eq!(root_node.child_nodes[0].consumer_address, addr_c1);
    assert!(root_node.child_nodes[0].child_nodes.is_empty());

    // Child 1: multi_children[0] → consumer = C2, leaf
    assert_eq!(root_node.child_nodes[1].consumer_address, addr_c2);
    assert!(root_node.child_nodes[1].child_nodes.is_empty());

    // Child 2: multi_children[1] → consumer = C3, leaf
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

    let unsigned = UnsignedSigner {
        consumer: addr_b,
        data: __groundhog_mintable_middle_auth::MiddleAuthUnsigned {
            label: 42,
            single_child: UnsignedSigner {
                consumer: addr_c1,
                data: __groundhog_mintable_leaf_auth::LeafAuthUnsigned {
                    data: 100,
                },
            },
            multi_children: vec![
                UnsignedSigner {
                    consumer: addr_c2,
                    data: __groundhog_mintable_leaf_auth::LeafAuthUnsigned {
                        data: 200,
                    },
                },
                UnsignedSigner {
                    consumer: addr_c3,
                    data: __groundhog_mintable_leaf_auth::LeafAuthUnsigned {
                        data: 300,
                    },
                },
            ],
        },
    };

    let root_node =
        <Signer<MiddleAuth> as Mintable>::build_auth_token_node(unsigned);

    // The root node's payload is WriteXDR of MiddleAuth
    // (Signer fields write 0 bytes, Vec length is preserved)
    let payload_bytes: &[u8] = root_node.payload.as_ref();

    // Decode the payload via ReadXDR — Signers get sentinel handles
    let mut decoded: MiddleAuth =
        xdrs::cxs::decode(payload_bytes).expect("payload decode failed");

    // Before ReceiveAuthHandles: signers have sentinel handles
    assert_eq!(decoded.single_child.handle(), u32::MAX);
    for s in &decoded.multi_children {
        assert_eq!(s.handle(), u32::MAX);
    }

    // Simulate what consume() does: fill in child handles
    // The runtime would return handles [10, 20, 30] for the 3 children
    let child_handles = vec![10u32, 20u32, 30u32];
    let mut iter = child_handles.into_iter();
    decoded.receive_auth_handles(&mut iter);

    // After ReceiveAuthHandles: signers have real handles
    assert_eq!(decoded.label, 42);
    assert_eq!(decoded.single_child.handle(), 10);
    assert_eq!(decoded.multi_children.len(), 2);
    assert_eq!(decoded.multi_children[0].handle(), 20);
    assert_eq!(decoded.multi_children[1].handle(), 30);
}

// ── Test: CollectAuthHandles extracts handles for forwarding ──

#[test]
fn collect_handles_for_forwarding() {
    // Simulate B has consumed: decode + receive handles (same as consume() does)
    // Start with default MiddleAuth (sentinel handles)
    let mut middle = MiddleAuth {
        label: 42,
        single_child: Signer::default(),
        multi_children: vec![Signer::default(), Signer::default()],
    };
    let mut iter = [10u32, 20u32, 30u32].into_iter();
    middle.receive_auth_handles(&mut iter);

    // CollectAuthHandles should extract [10, 20, 30]
    let mut buf = sdk::collect_auth_handles::AuthHandleBuf::new();
    middle.collect_auth_handles(&mut buf);
    assert_eq!(buf.as_slice(), &[10, 20, 30]);

    // Forwarding single_child to C via a method call:
    // The generated client code wraps args in a tuple and collects handles
    let call_args = (42u64, middle.single_child);
    let mut buf2 = sdk::collect_auth_handles::AuthHandleBuf::new();
    call_args.collect_auth_handles(&mut buf2);
    assert_eq!(buf2.as_slice(), &[10]);
}

// ── Test: leaf payload roundtrip ──

#[test]
fn leaf_payload_roundtrip() {
    let leaf_unsigned = UnsignedSigner {
        consumer: addr(0xCC),
        data: __groundhog_mintable_leaf_auth::LeafAuthUnsigned { data: 99 },
    };

    let node =
        <Signer<LeafAuth> as Mintable>::build_auth_token_node(leaf_unsigned);

    assert_eq!(node.consumer_address, addr(0xCC));
    assert!(node.child_nodes.is_empty());

    // Decode leaf payload
    let decoded: LeafAuth = xdrs::cxs::decode(node.payload.as_ref())
        .expect("leaf payload decode failed");
    assert_eq!(decoded.data, 99);
}

// ── Test: full encode_auth_token_node produces valid XDR ──

#[test]
fn encode_auth_token_node_roundtrip() {
    let unsigned = UnsignedSigner {
        consumer: addr(0xBB),
        data: __groundhog_mintable_leaf_auth::LeafAuthUnsigned { data: 77 },
    };

    let node = <Signer<LeafAuth> as Mintable>::build_auth_token_node(unsigned);

    // Encode the full AuthTokenNode (what gets passed to MintAuthToken syscall)
    let encoded = sdk::mintable::encode_auth_token_node(&node);
    assert!(!encoded.is_empty());

    // Decode it back
    let decoded: sdk::AuthTokenNode =
        xdrs::txs::decode(&encoded).expect("AuthTokenNode decode failed");
    assert_eq!(decoded.consumer_address, addr(0xBB));
    assert!(decoded.child_nodes.is_empty());
}
