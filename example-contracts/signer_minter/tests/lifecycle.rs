//! Full signer lifecycle test:
//! A (minter) mints a nested auth token → B (middle) consumes + forwards → C (leaf) consumes
//!
//! The token tree:
//!   MiddleAuth { label: 1000, child: Signer<LeafAuth{10}>, extra_children: [Signer<LeafAuth{20}>, Signer<LeafAuth{30}>] }
//!
//! Expected result: 1000 + 10 + 20 + 30 = 1060

use sdk::*;

#[test]
fn full_mint_consume_forward_lifecycle() {
    let env = sdk::TestEnvironment::no_cost();

    let leaf_mock = signer_leaf::register_mock_contract(&env);
    let middle_mock = signer_middle::register_mock_contract(&env);
    let minter_mock = signer_minter::register_mock_contract(&env);

    let leaf_addr: ContractAddress = [0xCC; 32].into();
    let middle_addr: ContractAddress = [0xBB; 32].into();
    let minter_addr: ContractAddress = [0xAA; 32].into();

    leaf_mock.deploy_contract_to(&leaf_addr);
    middle_mock.deploy_contract_to(&middle_addr);
    minter_mock.deploy_contract_to(&minter_addr);

    sdk::contract_exec(env, move || {
        let minter = signer_minter::SignerMinterClient::new(&minter_addr);
        let result = minter.mint_and_call(&middle_addr, &leaf_addr);
        // 1000 (label) + 10 (child) + 20 (extra[0]) + 30 (extra[1]) = 1060
        assert_eq!(result, 1060);
    })
    .unwrap_or_else(|e| panic!("Signer lifecycle test failed: {:?}", e));
}
