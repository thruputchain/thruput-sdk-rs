use recursive_type::*;
use thruput_sdk::*;

static NONCE: DeployNonce =
    DeployNonce::new(thruput_sdk::sha256_static!("RECURSIVE_TYPE_TEST"));

fn setup() -> (TestEnvironmentHandle, CodeHash<Client>) {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let test_contract = recursive_type::register_mock_contract(&env);
    let code = CodeHash::of_mock(&test_contract);
    (env, code)
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
    let (env, code) = setup();
    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let client = code.deploy(&NONCE).client();
            assert_eq!(client.depth(leaf(42)), 1);
        })
    })
    .assert_success();
}

#[test]
fn depth_of_balanced_tree() {
    let (env, code) = setup();
    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let client = code.deploy(&NONCE).client();

            //        1
            //       / \
            //      2   3
            //     / \
            //    4   5
            let tree = node(1, node(2, leaf(4), leaf(5)), leaf(3));
            assert_eq!(client.depth(tree), 3);
        })
    })
    .assert_success();
}

#[test]
fn depth_of_right_spine() {
    let (env, code) = setup();
    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let client = code.deploy(&NONCE).client();

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
    })
    .assert_success();
}

#[test]
fn forest_leaf_depth_is_one() {
    let (env, code) = setup();
    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let client = code.deploy(&NONCE).client();

            let f = Forest {
                trees: vec![],
                label: 1,
            };
            assert_eq!(client.forest_depth(f), 1);
        })
    })
    .assert_success();
}

#[test]
fn forest_nested_depth() {
    let (env, code) = setup();
    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let client = code.deploy(&NONCE).client();

            let f = Forest {
                label: 1,
                trees: vec![
                    Forest {
                        label: 2,
                        trees: vec![Forest {
                            label: 3,
                            trees: vec![],
                        }],
                    },
                    Forest {
                        label: 4,
                        trees: vec![],
                    },
                ],
            };
            assert_eq!(client.forest_depth(f), 3);
        })
    })
    .assert_success();
}

#[test]
fn unbox_u32() {
    let (env, code) = setup();
    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let client = code.deploy(&NONCE).client();
            assert_eq!(client.unbox(Box::new(99)), 99);
        })
    })
    .assert_success();
}
