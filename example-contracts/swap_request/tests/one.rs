use swap_request::*;

#[test]
fn test() {
    sdk::client_test(|env| env.register_mock(SwapContract::__mock), mytest);
}

fn mytest(c: &mut SwapClient) {
    c.submit_request(SwapRequest::new());
    c.submit_request(SwapRequest::new());
    c.submit_request(SwapRequest::new());
    c.submit_request(SwapRequest::new());

    let last_submitted = SwapRequest::last_id();

    let last_processed = c.process_requests();

    assert_eq!(last_processed, last_submitted);
}
