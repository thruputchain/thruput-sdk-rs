use recursive_type::*;
use sdk::*;

static NONCE: SHA256Hash = sdk::sha256_static!("RECURSIVE_TYPE_TEST");

fn setup() -> (TestEnvironmentHandle, SHA256Hash) {
    let env = sdk::TestEnvironment::no_cost();
    let test_contract = recursive_type::register_mock_contract(&env);
    let contract_hash = test_contract.as_hash().clone();
    (env, contract_hash)
}

fn leaf(value: u32) -> Tree {
    Tree {
        value,
        left: None,
        right: None,
    }
}

fn node(value: u32, left: Tree, right: Tree) -> Tree {
    Tree {
        value,
        left: Some(Box::new(left)),
        right: Some(Box::new(right)),
    }
}

#[test]
fn depth_of_leaf_is_one() {
    let (env, contract_hash) = setup();
    contract_exec(env, move || {
        let addr = deploy_contract(&NONCE, &contract_hash);
        let client = Client::new(&addr);
        assert_eq!(client.depth(leaf(42)), 1);
    })
    .unwrap();
}

#[test]
fn depth_of_balanced_tree() {
    let (env, contract_hash) = setup();
    contract_exec(env, move || {
        let addr = deploy_contract(&NONCE, &contract_hash);
        let client = Client::new(&addr);

        //        1
        //       / \
        //      2   3
        //     / \
        //    4   5
        let tree = node(1, node(2, leaf(4), leaf(5)), leaf(3));
        assert_eq!(client.depth(tree), 3);
    })
    .unwrap();
}

#[test]
fn depth_of_right_spine() {
    let (env, contract_hash) = setup();
    contract_exec(env, move || {
        let addr = deploy_contract(&NONCE, &contract_hash);
        let client = Client::new(&addr);

        let tree = Tree {
            value: 1,
            left: None,
            right: Some(Box::new(Tree {
                value: 2,
                left: None,
                right: Some(Box::new(Tree {
                    value: 3,
                    left: None,
                    right: Some(Box::new(leaf(4))),
                })),
            })),
        };
        assert_eq!(client.depth(tree), 4);
    })
    .unwrap();
}
