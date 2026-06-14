//! Auth-token minting through `#[contract_type]` enums: a `Signed<T>`
//! inside an enum variant must mint a child AuthTokenNode, exactly as
//! it does inside a struct field (see signer_lifecycle.rs).

#![allow(unexpected_cfgs)]

use sdk::mintable::{AuthTokenPreimage, MintableType, build_auth_token_node};
use sdk::receive_auth_handles::ReceiveAuthHandles;
use sdk::signer::Signed;
use sdk::*;

#[contract_type]
pub struct PermitData {
    pub amount: u64,
}

#[contract_type]
pub enum Action {
    Noop = 0,
    Delegated(Signed<PermitData>) = 1,
    Pair { id: u32, permit: Signed<PermitData> } = 2,
}

type ActionPreimage = <Action as MintableType>::Preimage;
type PermitPreimage = <PermitData as MintableType>::Preimage;

fn addr(byte: u8) -> ContractAddress {
    ContractAddress::from([byte; 32])
}

#[test]
fn unit_variant_mints_no_children() {
    let node = build_auth_token_node::<Action>(AuthTokenPreimage {
        consumer: addr(0xBB),
        data: ActionPreimage::Noop,
    });
    assert!(node.child_nodes.is_empty());
}

#[test]
fn tuple_variant_mints_child_token() {
    let node = build_auth_token_node::<Action>(AuthTokenPreimage {
        consumer: addr(0xBB),
        data: ActionPreimage::Delegated(AuthTokenPreimage {
            consumer: addr(0xCC),
            data: PermitPreimage { amount: 7 },
        }),
    });
    assert_eq!(node.consumer_address, addr(0xBB));
    assert_eq!(node.child_nodes.len(), 1);
    assert_eq!(node.child_nodes[0].consumer_address, addr(0xCC));
    assert!(node.child_nodes[0].child_nodes.is_empty());
}

#[test]
fn named_variant_mints_child_token() {
    let node = build_auth_token_node::<Action>(AuthTokenPreimage {
        consumer: addr(0xBB),
        data: ActionPreimage::Pair {
            id: 9,
            permit: AuthTokenPreimage {
                consumer: addr(0xCD),
                data: PermitPreimage { amount: 8 },
            },
        },
    });
    assert_eq!(node.child_nodes.len(), 1);
    assert_eq!(node.child_nodes[0].consumer_address, addr(0xCD));
}

#[test]
fn payload_roundtrip_with_receive_handles() {
    let node = build_auth_token_node::<Action>(AuthTokenPreimage {
        consumer: addr(0xBB),
        data: ActionPreimage::Delegated(AuthTokenPreimage {
            consumer: addr(0xCC),
            data: PermitPreimage { amount: 7 },
        }),
    });

    // Decode the minted payload (Signed encodes zero bytes; the data
    // travels in the child node) and bind a handle, mirroring
    // signer_lifecycle.rs.
    let payload_bytes: &[u8] = node.payload.as_ref();
    let mut decoded: Action =
        xdrs::cxs::decode(payload_bytes).expect("payload decode failed");

    let mut iter = [10u32].into_iter();
    decoded.receive_auth_handles(&mut iter).unwrap();

    match decoded {
        Action::Delegated(signed) => assert_eq!(signed.handle(), 10),
        _ => panic!("wrong variant decoded: discriminant mismatch"),
    }
}

#[contract_type]
pub enum Multi {
    Two {
        first: Signed<PermitData>,
        second: Signed<PermitData>,
    } = 0,
}

type MultiPreimage = <Multi as MintableType>::Preimage;

#[test]
fn two_signed_fields_mint_in_declaration_order() {
    let node = build_auth_token_node::<Multi>(AuthTokenPreimage {
        consumer: addr(0xBB),
        data: MultiPreimage::Two {
            first: AuthTokenPreimage {
                consumer: addr(0xC1),
                data: PermitPreimage { amount: 1 },
            },
            second: AuthTokenPreimage {
                consumer: addr(0xC2),
                data: PermitPreimage { amount: 2 },
            },
        },
    });
    assert_eq!(node.child_nodes.len(), 2);
    assert_eq!(node.child_nodes[0].consumer_address, addr(0xC1));
    assert_eq!(node.child_nodes[1].consumer_address, addr(0xC2));

    let mut decoded: Multi = xdrs::cxs::decode(node.payload.as_ref())
        .expect("payload decode failed");
    let mut iter = [21u32, 22u32].into_iter();
    decoded.receive_auth_handles(&mut iter).unwrap();
    let Multi::Two { first, second } = decoded;
    assert_eq!(first.handle(), 21);
    assert_eq!(second.handle(), 22);
}

#[contract_type]
pub enum Shadow {
    Holder { children: Signed<PermitData> } = 0,
}

type ShadowPreimage = <Shadow as MintableType>::Preimage;

#[test]
fn field_named_children_does_not_shadow_buffer() {
    let node = build_auth_token_node::<Shadow>(AuthTokenPreimage {
        consumer: addr(0xBB),
        data: ShadowPreimage::Holder {
            children: AuthTokenPreimage {
                consumer: addr(0xCE),
                data: PermitPreimage { amount: 3 },
            },
        },
    });
    assert_eq!(node.child_nodes.len(), 1);
    assert_eq!(node.child_nodes[0].consumer_address, addr(0xCE));
}

#[contract_type]
pub enum WalkerShadow {
    Both {
        out: Signed<PermitData>,
        handles: Signed<PermitData>,
    } = 0,
}

type WalkerShadowPreimage = <WalkerShadow as MintableType>::Preimage;

#[test]
fn fields_named_out_and_handles_walk_correctly() {
    let node = build_auth_token_node::<WalkerShadow>(AuthTokenPreimage {
        consumer: addr(0xBB),
        data: WalkerShadowPreimage::Both {
            out: AuthTokenPreimage {
                consumer: addr(0xD1),
                data: PermitPreimage { amount: 1 },
            },
            handles: AuthTokenPreimage {
                consumer: addr(0xD2),
                data: PermitPreimage { amount: 2 },
            },
        },
    });
    assert_eq!(node.child_nodes.len(), 2);

    let mut decoded: WalkerShadow = xdrs::cxs::decode(node.payload.as_ref())
        .expect("payload decode failed");
    let mut iter = [31u32, 32u32].into_iter();
    decoded.receive_auth_handles(&mut iter).unwrap();
    let WalkerShadow::Both { out, handles } = decoded;
    assert_eq!(out.handle(), 31);
    assert_eq!(handles.handle(), 32);
}
