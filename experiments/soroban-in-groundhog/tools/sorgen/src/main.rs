//! Emits hex calldata (XDR HostFunction) for the sorhog guest, and decodes
//! XDR ScVal return values.
use sha2::{Digest, Sha256};
use stellar_xdr::*;

const NETWORK_ID: [u8; 32] = [7; 32]; // matches the guest's LedgerInfo
fn source() -> ScAddress {
    ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256([1; 32]))))
}
fn wasm_hash(wasm: &[u8]) -> Hash { Hash(Sha256::digest(wasm).into()) }
fn preimage() -> ContractIdPreimage {
    ContractIdPreimage::Address(ContractIdPreimageFromAddress { address: source(), salt: Uint256([0; 32]) })
}
fn contract_addr() -> ScAddress {
    let pre = HashIdPreimage::ContractId(HashIdPreimageContractId {
        network_id: Hash(NETWORK_ID),
        contract_id_preimage: preimage(),
    });
    let id: [u8; 32] = Sha256::digest(pre.to_xdr(Limits::none()).unwrap()).into();
    ScAddress::Contract(ContractId(Hash(id)))
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let hf = match a[1].as_str() {
        "upload" => HostFunction::UploadContractWasm(std::fs::read(&a[2]).unwrap().try_into().unwrap()),
        "create" => HostFunction::CreateContractV2(CreateContractArgsV2 {
            contract_id_preimage: preimage(),
            executable: ContractExecutable::Wasm(wasm_hash(&std::fs::read(&a[2]).unwrap())),
            constructor_args: VecM::default(),
        }),
        "increment" => HostFunction::InvokeContract(InvokeContractArgs {
            contract_address: contract_addr(),
            function_name: "increment".try_into().unwrap(),
            args: vec![ScVal::U32(a[2].parse().unwrap())].try_into().unwrap(),
        }),
        "decode" => {
            let v = ScVal::from_xdr(hex::decode(a[2].trim_start_matches("0x")).unwrap(), Limits::none()).unwrap();
            println!("{v:?}");
            return;
        }
        _ => panic!("upload|create|increment|decode"),
    };
    let x = hf.to_xdr(Limits::none()).unwrap();
    eprintln!("{} bytes", x.len());
    println!("0x{}", hex::encode(x));
}
