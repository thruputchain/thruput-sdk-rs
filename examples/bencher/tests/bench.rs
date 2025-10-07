use sdk::*;
use sdk::contract::ContractBlob;
use sdk::sysresult::Result;
use bencher::BencherClient;

// working directory for tests is the (test) package root
const ERC20_PATH: &str = "../../target/optimized/erc20.wasm";

#[test]
fn test() {
    let mut env = sdk::TestEnvironment::new();
    sdk::TestEnvironment::enable_syscall_logging();

    // Setup "bencher" as a native contract
    let bencher = bencher::test_setup::register(&mut env);
    let bencher_hash = env.setup_create_contract(&bencher);

    // Load erc20.wasm and setup the contract
    let erc20_bytes = std::fs::read(ERC20_PATH).unwrap_or_else(|e|
        panic!("couldn't load wasm to test: {}", e));
    let erc20_blob: ContractBlob = BytesM(erc20_bytes);
    let erc20 = Contract::kWasmi {
        wasmi_contract: erc20_blob,
    };
    let erc20_hash = env.setup_create_contract(&erc20);

    let mut handler = env.prepare_test(move || -> () {
        match test_inner(&bencher_hash, &erc20_hash) {
            Ok(_) => (),
            Err(error) => panic!("test failed: {}", error),
        }
    });
    let res = handler.root_invoke();

    match res.result.panic.try_into() {
        Ok(InvokeErrorCode::kInvokeNone) => { /* success */ },
        Ok(invoke_err) => panic!("test-invoke failed ({:?})", invoke_err),
        Err(_) => panic!("test-invoke failed with unknown error ({})", res.result.panic),
    }
}

// Deploy the two contracts and then call bencher with the address of the ERC20 contract
fn test_inner(bencher_hash: &SHA256Hash, erc20_hash: &SHA256Hash) -> Result<()> {
    let bencher_nonce = SHA256Hash::from([7; 32]);
    let bencher_addr = deploy_contract(&bencher_nonce, bencher_hash)?;

    let erc20_nonce = SHA256Hash::from([8; 32]);
    let erc20_addr = deploy_contract(&erc20_nonce, erc20_hash)?;

    let mut bencher = BencherClient::new(&bencher_addr);

    bencher.bench_erc20(&erc20_addr)?;

    Ok(())
}
