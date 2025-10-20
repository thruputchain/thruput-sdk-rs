use groundhog_xdr::contract::*;
use groundhog_xdr::ffi_enums::InvokeErrorCode;
use groundhog_xdr::transaction::*;
use groundhog_xdr::types::*;
use sdk::*;
use sdk::contract_test::load_wasm_contract;
use native_exec::transaction_handler::TransactionHandler;
use xdrs::serialize_txs;
use sha2::{Sha256, Digest};

pub fn main() {

    let package = std::env::args()
        .skip(1)
        .next()
        .unwrap_or_else(|| panic!("Please specify contract package to test"));

    bottle_test(&package);
}

fn bottle_test(package: &str) {

    let target_contract =
        load_wasm_contract(format!("target/optimized/{package}.wasm"));

    let tester_contract =
        load_wasm_contract(format!("target/bottle-test/{package}.wasm"));

    let env = TestEnvironment::new();

    // Create target contract.  The tester can then deploy the target contract.
    // TODO: Should we do it anyway, to remove the deployment from gas calculations?
    // Or perhaps the test interface should include an initialization method, to be
    // run before run().
    let target_hash = env.setup_create_contract(&target_contract);

    // Create and deploy tester contract
    let _tester_hash = env.setup_create_contract(&tester_contract);
    let tester_addr = contract_deploy_address(&tester_contract);
    env.setup_create_deployment(&tester_addr, &tester_contract);

    // Construct the call `tester.run(&target_hash)`
    let entrypoint = contract_entrypoint_id(
        Some("BottleTest"),
        "0.0.0",
        "run",
        0,
    );
    let args = &target_hash;
    let stx = make_signed_transaction(&tester_addr, entrypoint, args);

    // Construct the transaction
    let mut run_test = TransactionHandler::new(env.clone(), &stx);

    // Run the transaction
    let gas_consumed = run_transaction(&mut run_test).unwrap_or_else(|e|
        panic!("root invocation panicked with code {}", e));

    println!(">> Bottle test completed successfully (gas consumed: {})", gas_consumed);
}

//
// Remainder of file: general test utilities
//

fn run_transaction(tx: &mut TransactionHandler) -> Result<u64, i32> {
    match tx.root_invoke() {
        r if r.result.panic == InvokeErrorCode::kInvokeNone as i32 => Ok(r.gas_consumed),
        r => Err(r.result.panic),
    }
}

fn make_signed_transaction(
    recipient_address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    args: &impl WriteXDR,
) -> SignedTransaction {

    SignedTransaction {
        signatures: vec![].try_into().unwrap(),
        tx: Transaction {
            uniquifier: Uniquifier {
                block_expiry_height: 0,
                nonce: 0,
            },
            gas_claims: [].try_into().unwrap(),
            auth_claims: [].try_into().unwrap(),
            invocation: Invocation {
                invoked_address: *recipient_address,
                entrypoint: entrypoint,
                gas_limit: u64::MAX,
                calldata: serialize_txs::encode(args)
                    .unwrap().try_into().unwrap(),
            },
            contracts_to_deploy: vec![].try_into().unwrap(),
        },
    }
}

fn contract_deploy_address(_contract: &Contract) -> ContractAddress {
    [0xFF; 32].into()

    /* ... i guess we don't have to bother with all this ...
    let sender_addr = ContractAddress::default(); // XXX: ?
    let tester_nonce = SHA256Hash::from([8; 32]);
    let tester_bytes = serialize_txs::encode(&tester_contract).unwrap();
    let tester_hash = Sha256::digest(tester_bytes.as_slice());
    let tester_hash: [u8; 32] = tester_hash.into();
    let tester_hash: SHA256Hash = tester_hash.into();
    let tester_addr = get_deploy_address(&sender_addr, &tester_nonce, &tester_hash);
    */
}

#[allow(unused)]
fn get_deploy_address(
    sender: &ContractAddress,
    nonce: &SHA256Hash,
    contract_hash: &SHA256Hash,
) -> ContractAddress {

    let preimage = ContractDeploymentHashPreimage {
        deployer_address: *sender,
        nonce: *nonce,
        contract_hash: *contract_hash
    };
    let encoding = serialize_txs::encode(&preimage).unwrap();
    let hash = Sha256::digest(encoding.as_slice());
    let arr: [u8; 32] = hash.into();
    arr.into()
}

fn contract_entrypoint_id(
    interface_name: Option<&str>,
    version_str: &str,
    method_name: &str,
    method_index: u16,
) -> ContractEntrypointID {

    let iface_name = interface_name.unwrap_or(method_name);

    let preimage = ContractInterfaceIDPreimage {
        name: iface_name.as_bytes().try_into().unwrap(),
        vers: version_str.as_bytes().try_into().unwrap(),
    };

    let preimage = serialize_txs::encode(&preimage).unwrap();

    let preimage_hash = Sha256::digest(preimage.as_slice());

    let interface_id: &[u8; 6] = &preimage_hash[0..6].try_into().unwrap();

    let method_id: [u8; 2] = method_index.to_be_bytes();

    let mut entrypoint_raw = [0; 8];
    entrypoint_raw[0..6].copy_from_slice(interface_id);
    entrypoint_raw[6..8].copy_from_slice(&method_id);

    let id = ContractEntrypointID::from(entrypoint_raw);

    println!("  ENTRYPOINT({}_v{}::method_{}) -> {}",
        iface_name, version_str, method_index, entrypoint_export_name(id));

    id
}

fn entrypoint_export_name(id: ContractEntrypointID) -> String {
    format!("{:016X}", u64::from_be_bytes(*id))
}
