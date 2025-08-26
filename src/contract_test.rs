// TODO: fail the test on panic

#[macro_export]
macro_rules! contract_test {
    ($name:ident $test_body:tt) => {
        #[test]
        fn $name() {
            sdk::TestEnvironment::enable_syscall_logging();

            let mut env = sdk::TestEnvironment::new();

            let test_contract = {
                let mut contract = env.add_contract();
                let method = sdk::types::ContractEntrypointID::from(1u64.to_be_bytes());
                contract.register_method(method, || -> () {
                    sdk::handle::handle_call(|()| $test_body);
                });
                contract.as_contract()
            };

            let contract_hash = env.setup_create_contract(&test_contract);

            let mut handler = env.prepare_test(move || -> () {
                let nonce = sdk::types::SHA256Hash::from([7; 32]);
                let deployed_addr =
                    sdk::deploy::deploy_contract(&nonce, &contract_hash)
                        .unwrap();

                // Invoke contract method 1
                let method = sdk::types::ContractEntrypointID::from(1u64.to_be_bytes());
                let _ = sdk::invoke::invoke::<(), ()>(
                    &deployed_addr,
                    method,
                    u64::MAX,
                    &(),
                );
            });

            let res = handler.root_invoke();
            assert_eq!(
                res.result.panic,
                sdk::InvokeErrorCode::kInvokeNone as i32
            );
        }
    };
}
