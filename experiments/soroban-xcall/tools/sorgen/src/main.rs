//! Calldata for the soroban-xcall Groundhog contracts, and result decoding.
//!
//!   sorgen call <contract-hex> <fn> [u32:N | u64:N | addr:<hex> | bytes:<hex>]...
//!       -> 0x<XDR HostFunction::InvokeContract> (method 0 calldata)
//!   sorgen decode 0x<XDR ScVal>
//!   sorgen gh-method <Interface> <version> <method>  -> Groundhog entrypoint id (u64)
//!   sorgen gh-u64 <n>                               -> CXS calldata for one u64 arg
//!   sorgen gh-addr-u32 <contract-hex> <n>           -> CXS calldata for (ContractAddress, u32)
//!   sorgen entry <spec.json>                        -> calldata for the trusted entrypoint
//!
//! Contract/address hex may be written `@name`, read from out/addr_<name>.
//! Entry spec: {"target": C, "fn": F, "args": [ARG...], "auth": [NODE...]}
//!   NODE = {"authorizer": ADDR, "contract": C, "fn": F, "args": [ARG...],
//!           "children": [NODE...]}
//!
//! A Soroban contract id is its Groundhog contract's address.
use stellar_xdr::*;

fn bytes32(s: &str) -> [u8; 32] {
    let hex = match s.strip_prefix('@') {
        Some(name) => std::fs::read_to_string(format!("out/addr_{name}")).expect("unknown @name"),
        None => s.to_string(),
    };
    hex::decode(hex.trim().trim_start_matches("0x")).unwrap().try_into().unwrap()
}

fn contract(s: &str) -> ScAddress {
    ScAddress::Contract(ContractId(Hash(bytes32(s))))
}

fn arg(a: &str) -> ScVal {
    let (ty, v) = a.split_once(':').expect("typed arg, e.g. u32:5");
    match ty {
        "u32" => ScVal::U32(v.parse().unwrap()),
        "u64" => ScVal::U64(v.parse().unwrap()),
        "i128" => {
            let n: i128 = v.parse().unwrap();
            ScVal::I128(Int128Parts { hi: (n >> 64) as i64, lo: n as u64 })
        }
        "addr" => ScVal::Address(contract(v)),
        "bytes" => ScVal::Bytes(hex::decode(v.trim_start_matches("0x")).unwrap().try_into().unwrap()),
        _ => panic!("unknown arg type {ty}"),
    }
}

fn args_of(v: &serde_json::Value) -> Vec<ScVal> {
    v.as_array().unwrap().iter().map(|a| arg(a.as_str().unwrap())).collect()
}

/// 8-byte call fingerprint; must match the guest's `auth::fingerprint`.
fn fingerprint(contract_hex: &str, func: &str, args: &[ScVal]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let ica = InvokeContractArgs {
        contract_address: contract(contract_hex),
        function_name: func.try_into().unwrap(),
        args: args.to_vec().try_into().unwrap(),
    };
    Sha256::digest(ica.to_xdr(Limits::none()).unwrap())[..8].to_vec()
}

/// One auth tree node -> Groundhog AuthTokenNode (payload: XDR ScVal::Vec
/// [authorizer, fn, args, child fingerprints]).
fn token_node(n: &serde_json::Value) -> groundhog_xdr::auth_token::AuthTokenNode {
    let children: Vec<serde_json::Value> =
        n.get("children").and_then(|c| c.as_array()).cloned().unwrap_or_default();
    let mut fps = Vec::new();
    for c in &children {
        fps.extend(fingerprint(c["contract"].as_str().unwrap(), c["fn"].as_str().unwrap(), &args_of(&c["args"])));
    }
    let payload = ScVal::Vec(Some(
        vec![
            arg(n["authorizer"].as_str().unwrap()),
            ScVal::Symbol(n["fn"].as_str().unwrap().try_into().unwrap()),
            ScVal::Vec(Some(args_of(&n["args"]).try_into().unwrap())),
            ScVal::Bytes(fps.try_into().unwrap()),
        ]
        .try_into()
        .unwrap(),
    ))
    .to_xdr(Limits::none())
    .unwrap();
    eprintln!("auth node {} payload: {} bytes (limit 256)", n["fn"], payload.len());
    let payload: xdrs::BytesM<256> = payload.as_slice().try_into().expect("payload over 256 bytes");
    groundhog_xdr::auth_token::AuthTokenNode {
        consumer_address: groundhog_xdr::types::ContractAddress::from(bytes32(n["contract"].as_str().unwrap())),
        payload: payload.into(),
        child_nodes: children.iter().map(token_node).collect::<Vec<_>>().as_slice().try_into().expect("over 4 children"),
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a[1].as_str() {
        "call" => {
            let hf = HostFunction::InvokeContract(InvokeContractArgs {
                contract_address: contract(&a[2]),
                function_name: a[3].as_str().try_into().unwrap(),
                args: a[4..].iter().map(|s| arg(s)).collect::<Vec<_>>().try_into().unwrap(),
            });
            println!("0x{}", hex::encode(hf.to_xdr(Limits::none()).unwrap()));
        }
        "decode" => {
            let v = ScVal::from_xdr(hex::decode(a[2].trim_start_matches("0x")).unwrap(), Limits::none()).unwrap();
            println!("{v:?}");
        }
        "entry" => {
            let spec: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&a[2]).unwrap()).unwrap();
            let target = spec["target"].as_str().unwrap();
            let hf = HostFunction::InvokeContract(InvokeContractArgs {
                contract_address: contract(target),
                function_name: spec["fn"].as_str().unwrap().try_into().unwrap(),
                args: args_of(&spec["args"]).try_into().unwrap(),
            })
            .to_xdr(Limits::none())
            .unwrap();
            let mut out = bytes32(target).to_vec();
            out.extend((hf.len() as u32).to_be_bytes());
            out.extend(hf);
            let auth = spec["auth"].as_array().cloned().unwrap_or_default();
            out.extend((auth.len() as u32).to_be_bytes());
            for n in &auth {
                let node = xdrs::txs::encode(&token_node(n)).unwrap();
                out.extend((node.len() as u32).to_be_bytes());
                out.extend(node);
            }
            println!("0x{}", hex::encode(out));
        }
        "gh-method" => {
            let sig = groundhog_contract_util::NormalizedMethodSignature::new(&a[4]);
            let id = groundhog_contract_util::contract_entrypoint_id(Some(&a[2]), &a[3], &sig);
            let b: [u8; 8] = id.as_ref()[..8].try_into().unwrap();
            println!("{}", u64::from_be_bytes(b));
        }
        "gh-u64" => {
            let n: u64 = a[2].parse().unwrap();
            println!("{}", hex::encode(xdrs::cxs::encode(&(n,)).unwrap()));
        }
        "gh-addr-u32" => {
            let b: [u8; 32] = hex::decode(a[2].trim_start_matches("0x")).unwrap().try_into().unwrap();
            let n: u32 = a[3].parse().unwrap();
            let addr = groundhog_xdr::types::ContractAddress::from(b);
            println!("{}", hex::encode(xdrs::cxs::encode(&(addr, n)).unwrap()));
        }
        _ => panic!("call|decode|entry|gh-method|gh-u64|gh-addr-u32"),
    }
}
