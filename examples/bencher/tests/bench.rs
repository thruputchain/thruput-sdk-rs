// use erc20_if::Result as ERC20Result;
// use erc20_if::ERC20Client;
use sdk::prelude::*;
use xdrs::BytesM;
use bencher::BencherClient;

const ERC20_PATH: &str = "/home/dbg/gh/all/sdk_rs/target/optimized/erc20.wasm";

#[test]
fn test() {
    let mut env = sdk::TestEnvironment::new();
    sdk::TestEnvironment::enable_syscall_logging();

    let bencher = bencher::test_setup::register(&mut env);
    let bencher_hash = env.setup_create_contract(&bencher);

    let erc20_bytes = std::fs::read(ERC20_PATH).unwrap();
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
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
}

fn test_inner(bencher_hash: &SHA256Hash, erc20_hash: &SHA256Hash) -> Result<()> {

    let bencher_nonce = SHA256Hash::from([7; 32]);
    let bencher_addr = deploy_contract(&bencher_nonce, bencher_hash)?;

    let erc20_nonce = SHA256Hash::from([8; 32]);
    let erc20_addr = deploy_contract(&erc20_nonce, erc20_hash)?;

    let mut bencher = BencherClient::new(&bencher_addr);

    bencher.test_erc20(&erc20_addr);

    Ok(())
}
