use groundhog_xdr::contract::*;
use groundhog_xdr::ffi_enums::InvokeErrorCode;
use groundhog_xdr::transaction::*;
use groundhog_xdr::types::*;
use native_exec::transaction_handler::TransactionHandler;
use native_exec::test_environment::TestEnvironmentHandle;
use sdk::contract_test::load_wasm_contract;
use sdk::*;
use sha2::{Digest, Sha256};
use xdrs::serialize_txs;
use hex_literal::*;

pub fn main() {
    let package = std::env::args()
        .skip(1)
        .next()
        .unwrap_or_else(|| panic!("Please specify contract package to test"));

    bottle_test(&package);
}

const BOTTLE_TEST_SETUP: u16 = 1;
const BOTTLE_TEST_RUN: u16 = 0;

fn bottle_test(package: &str) {
    let target_contract =
        load_wasm_contract(format!("target/optimized/{package}.wasm"));

    let tester_contract =
        load_wasm_contract(format!("target/bottle-test/{package}.wasm"));

    let env = TestEnvironment::new();

    // Create the target contract
    let target_hash = env.setup_create_contract(&target_contract);

    // Create and deploy the tester contract
    let _tester_hash = env.setup_create_contract(&tester_contract);
    let tester_addr = contract_deploy_address(&tester_contract);
    env.setup_create_deployment(&tester_addr, &tester_contract);

    // Invoke the `setup` method on the tester contract, allowing it to, e.g.,
    // deploy the target contract before running the test
    let setup_entrypoint = contract_entrypoint_id(Some("BottleTest"), "0.0.0", "setup", BOTTLE_TEST_SETUP);
    let setup_args = &target_hash;
    let _ = perform_contract_call(&env, &tester_addr, setup_entrypoint, setup_args)
        .unwrap_or_else(|e| panic!("bottle-test setup failed: {}", e));

    println!("BOTTLE SETUP OK");

    // Use the appropriate target address, found in the syscall traces
    let t: [u8; 32] = hex!("c5d9562c2d5462e49f8afb2ddfbd7d25932adbe061745b6b543a413cf6943f20");
    let target_address = ContractAddress::from(t);

    // Invoke the `run` method on the tester contract
    let run_entrypoint = contract_entrypoint_id(Some("BottleTest"), "0.0.0", "run", BOTTLE_TEST_RUN);
    let run_args = &(target_address,);
    let gas_consumed = perform_contract_call(&env, &tester_addr, run_entrypoint, run_args)
        .unwrap_or_else(|e| panic!("bottle-test run failed: {}", e));

    println!(
        ">> Bottle test completed successfully (gas consumed: {})",
        gas_consumed
    );
}

//
// Remainder of file: general test utilities
//

type InvErr = i32;

fn perform_contract_call(
    env: &TestEnvironmentHandle,
    contract_address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    args: &impl WriteXDR,
) -> Result<GasAmount, InvErr>
{
    // Construct the call
    let stx = make_signed_transaction(contract_address, entrypoint, args);

    // Construct the transaction
    let mut run_test = TransactionHandler::new(env.clone(), &stx);

    // Run the transaction
    run_transaction(&mut run_test)
}

fn run_transaction(tx: &mut TransactionHandler) -> Result<GasAmount, InvErr> {
    match tx.root_invoke() {
        r if r.result.panic == InvokeErrorCode::kInvokeNone as i32 => {
            Ok(r.gas_consumed)
        }
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
                    .unwrap()
                    .try_into()
                    .unwrap(),
            },
            contracts_to_deploy: vec![].try_into().unwrap(),
        },
    }
}

fn contract_deploy_address(contract: &Contract) -> ContractAddress {
    let sender_addr = ContractAddress::default();
    let nonce = SHA256Hash::from([8; 32]);

    let contract_bytes = serialize_txs::encode(&contract).unwrap();
    let contract_hash = Sha256::digest(contract_bytes.as_slice());
    let contract_hash: [u8; 32] = contract_hash.into();
    let contract_hash: SHA256Hash = contract_hash.into();

    get_deploy_address(&sender_addr, &nonce, &contract_hash)
}

fn get_deploy_address(
    sender: &ContractAddress,
    nonce: &SHA256Hash,
    contract_hash: &SHA256Hash,
) -> ContractAddress {
    let preimage = ContractDeploymentHashPreimage {
        deployer_address: *sender,
        nonce: *nonce,
        contract_hash: *contract_hash,
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

    /*
    println!(
        "  ENTRYPOINT({}_v{}::method_{}) -> {}",
        iface_name,
        version_str,
        method_index,
        entrypoint_export_name(id)
    );
    */

    id
}

#[allow(unused)]
fn entrypoint_export_name(id: ContractEntrypointID) -> String {
    format!("{:016X}", u64::from_be_bytes(*id))
}
