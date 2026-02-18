use sdk::transaction::InvocationFrameStatusCode;
mod common;
use common::*;

// Load two contracts into the same test environment, and
// have one call the other.
#[test]
fn caller_test_native() {
    let mut env = sdk::TestEnvironment::default();

    let erc20_contract = erc20::test_setup::register(&mut env);
    let erc20_hash = env.setup_create_contract(&erc20_contract);

    let caller_contract = caller::test_setup::register(&mut env);
    let caller_hash = env.setup_create_contract(&caller_contract);

    let mut handler = env.prepare_test(move || -> () {
        caller_test_inner(&erc20_hash, &caller_hash);
    });

    let res = handler.root_invoke();
    assert_eq!(
        res.output.status_code(),
        InvocationFrameStatusCode::kInvocationFrameStatusCodeReturn as i32
    );
}
