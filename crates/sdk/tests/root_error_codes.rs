use sdk::{ContractAddress, Named, TestEnvironment};

#[test]
fn test_root_frame_revert() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("root_error_codes"))
            .run(|| -> () {
                sdk::sys::raw_Return(&[], true, &[]);
            })
    });

    result.assert_abort();
    assert_eq!(
        *result.exec_result(),
        sdk::ExecResult::ExecResultTxRootFrameReverted
    );
}
