use crate::implementation;
use crate::interface::client::FibonacciClient;
use crate::interface::Fibonacci;
use sdk::prelude::*;

#[test]
fn test_fibonacci() {
    let mut env = sdk::TestEnvironment::new();

    let test_contract = implementation::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);
    
    sdk::TestEnvironment::enable_syscall_logging();

    let mut handler = env.prepare_test(move || -> () {
        match test_fibonacci_inner(&contract_hash) {
            Ok(_) => (),
            Err(error) => panic!("test failed: {}", error),
        }
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
}

fn test_fibonacci_inner(contract_hash: &sdk::types::SHA256Hash) -> Result<()> {
    let nonce = SHA256Hash::from([7; 32]);
    let deployed_addr = deploy_contract(&nonce, contract_hash)?;

    let mut client = FibonacciClient::new(&deployed_addr);
    FibonacciClient::initialize(Some(&mut client))?;

    assert_eq!(client.fibonacci(0), 0);
    assert_eq!(client.fibonacci(1), 1);
    assert_eq!(client.fibonacci(2), 1);
    assert_eq!(client.fibonacci(3), 2);
    assert_eq!(client.fibonacci(4), 3);
    assert_eq!(client.fibonacci(5), 5);
    assert_eq!(client.fibonacci(6), 8);

    assert_eq!(client.slow_fibonacci(0), 0);
    assert_eq!(client.slow_fibonacci(1), 1);
    assert_eq!(client.slow_fibonacci(2), 1);
    assert_eq!(client.slow_fibonacci(3), 2);
    assert_eq!(client.slow_fibonacci(4), 3);
    assert_eq!(client.slow_fibonacci(5), 5);
    assert_eq!(client.slow_fibonacci(6), 8);

    assert_eq!(client.fibonacci(10), client.slow_fibonacci(10));
    assert_eq!(client.fibonacci(40), client.slow_fibonacci(40));

    Ok(())
}
