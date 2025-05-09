#[macro_export]
macro_rules! contract_test {
    ($name:ident $test_body:tt) => {
        #[test]
        fn $name() {
            let mut env = sdk::TestEnvironment::new();

            let test_contract = {
                let mut contract = env.add_contract();
                contract.register_method(1, || -> i32 {
                    sdk::internal::handle_call(|()| $test_body).into()
                });
                contract.as_contract()
            };

            let contract_hash = env.setup_create_contract(&test_contract);

            let mut handler = env.prepare_test(move || -> i32 {
                let nonce = sdk::types::SHA256Hash::from(sdk::types::ByteArray::from([7; 32]));
                let deployed_addr =
                    sdk::deploy::deploy_contract(&nonce, &contract_hash)
                        .unwrap();

                // Invoke contract method 1
                let _ = sdk::internal::invoke::<(), ()>(
                    &deployed_addr,
                    1,
                    u64::MAX,
                    &(),
                );

                0
            });

            let res = handler.root_invoke();
            assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
        }
    };
}
