use crate::types::MockContract;
use groundhog_xdr::contract::Contract;
use groundhog_xdr::transaction::{Invocation, SignedTransaction, Transaction};
use groundhog_xdr::types::SHA256Hash;
use groundhog_xdr::{ByteArray, Limits, WriteXdr};
use sha2::{Digest, Sha256};

pub fn make_deploy_transaction(
    contract_code: Contract,
    nonce: &SHA256Hash,
) -> SignedTransaction {
    let mut hasher = Sha256::new();
    hasher.update(contract_code.to_xdr(Limits::none()).unwrap());
    let contract_hash = hasher.finalize();

    let concatenated: [u8; 64] = {
        let mut arr = [0u8; 64];
        arr[..32].copy_from_slice(nonce.as_slice());
        arr[32..].copy_from_slice(&contract_hash);
        arr
    };

    SignedTransaction {
        tx: Transaction {
            invocation: Invocation {
                invoked_address: ByteArray::new([0; 32]),
                method: MockContract::method_int_from_sdk_name("deploy"),
                gas_limit: u64::MAX,
                calldata: concatenated.try_into().unwrap(),
            },
            contracts_to_deploy: vec![contract_code].try_into().unwrap(),
        },
    }
}

pub fn make_deploy_and_invoke_transaction(
    contract_code: Contract,
    nonce: &SHA256Hash,
    method: u32,
    calldata: &[u8],
) -> SignedTransaction {
    let mut hasher = Sha256::new();
    hasher.update(contract_code.to_xdr(Limits::none()).unwrap());
    let contract_hash = hasher.finalize();

    let mut c: Vec<u8> = vec![];
    c.extend(nonce.as_slice());
    c.extend(contract_hash);
    c.extend(u32::to_le_bytes(method));
    let calldata_len: u8 = calldata.len() as u8;
    if calldata_len > 127 {
        panic!("unimplemented");
    }
    c.push(calldata_len);
    c.extend(calldata);

    SignedTransaction {
        tx: Transaction {
            invocation: Invocation {
                invoked_address: ByteArray::new([0; 32]),
                method: MockContract::method_int_from_sdk_name(
                    "deploy_and_invoke",
                ),
                gas_limit: u64::MAX,
                calldata: c.try_into().unwrap(),
            },
            contracts_to_deploy: vec![contract_code].try_into().unwrap(),
        },
    }
}
