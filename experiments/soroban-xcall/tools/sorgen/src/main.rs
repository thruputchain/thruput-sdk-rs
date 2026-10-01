//! Calldata for the soroban-xcall Groundhog contracts, and result decoding.
//!
//!   sorgen call <contract-hex> <fn> [u32:N | u64:N | addr:<hex> | bytes:<hex>]...
//!       -> 0x<XDR HostFunction::InvokeContract> (method 0 calldata)
//!   sorgen decode 0x<XDR ScVal>
//!   sorgen gh-method <Interface> <version> <method>  -> Groundhog entrypoint id (u64)
//!   sorgen gh-u64 <n>                               -> CXS calldata for one u64 arg
//!   sorgen gh-addr-u32 <contract-hex> <n>           -> CXS calldata for (ContractAddress, u32)
//!
//! A Soroban contract id is its Groundhog contract's address.
use stellar_xdr::*;

fn contract(hex32: &str) -> ScAddress {
    let b: [u8; 32] = hex::decode(hex32.trim_start_matches("0x")).unwrap().try_into().unwrap();
    ScAddress::Contract(ContractId(Hash(b)))
}

fn arg(a: &str) -> ScVal {
    let (ty, v) = a.split_once(':').expect("typed arg, e.g. u32:5");
    match ty {
        "u32" => ScVal::U32(v.parse().unwrap()),
        "u64" => ScVal::U64(v.parse().unwrap()),
        "addr" => ScVal::Address(contract(v)),
        "bytes" => ScVal::Bytes(hex::decode(v.trim_start_matches("0x")).unwrap().try_into().unwrap()),
        _ => panic!("unknown arg type {ty}"),
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
        _ => panic!("call|decode|gh-method|gh-u64|gh-addr-u32"),
    }
}
