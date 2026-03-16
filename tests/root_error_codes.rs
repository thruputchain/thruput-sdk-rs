use groundhog_xdr::transaction::InvocationFrameStatusCode;
use sdk::TestEnvironment;

#[test]
fn test_root_frame_revert() {
    let env = TestEnvironment::new(None);

    let mut handler = env.prepare_test(|| {
        sdk::sys::raw_Return(&[], true);
    });

    let res = handler.root_invoke();
    assert_eq!(
        res.output.status_code(),
        InvocationFrameStatusCode::kInvocationFrameStatusCodeAbort.0
    );

    let commit_res = handler.try_commit_transaction();
    assert_eq!(commit_res, sdk::ExecResult::ExecResultTxRootFrameReverted);
}
