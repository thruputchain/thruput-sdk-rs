use groundhog_native_exec::frame_result::FrameResult;
use groundhog_xdr::transaction::InvocationFrameOutput;

#[test]
fn test_frame_result_success() {
    let return_data = vec![1u8, 2, 3];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeReturn {
        return_buffer: return_data.clone().try_into().unwrap(),
    };
    let result = FrameResult::new(output, None);

    assert!(result.is_success());
    assert!(!result.is_abort());
    assert!(!result.is_fault());
    result.assert_success();
    assert_eq!(result.return_buffer(), &[1, 2, 3]);
}

#[test]
fn test_frame_result_abort() {
    let abort_data = vec![4u8, 5, 6];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeAbort {
        abort_buffer: abort_data.clone().try_into().unwrap(),
    };
    let result = FrameResult::new(output, None);

    assert!(result.is_abort());
    assert!(!result.is_success());
    result.assert_abort();
    assert_eq!(result.abort_buffer(), &[4, 5, 6]);
}

#[test]
fn test_frame_result_fault() {
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeFault;
    let result = FrameResult::new(output, None);

    assert!(result.is_fault());
    result.assert_fault();
}

#[test]
#[should_panic(expected = "expected success")]
fn test_frame_result_assert_success_panics_on_fault() {
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeFault;
    let result = FrameResult::new(output, None);
    result.assert_success();
}

#[test]
#[should_panic(expected = "expected abort")]
fn test_frame_result_assert_abort_panics_on_success() {
    let return_data = vec![];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeReturn {
        return_buffer: return_data.try_into().unwrap(),
    };
    let result = FrameResult::new(output, None);
    result.assert_abort();
}
