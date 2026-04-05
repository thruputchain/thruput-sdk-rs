use swap_request::*;

#[test]
fn test() {
    sdk::client_test(|env| swap_request::register_mock_contract(env), mytest);
}

fn mytest(c: &SwapClient) {
    c.submit_request(SwapRequest::new());
    c.submit_request(SwapRequest::new());
    c.submit_request(SwapRequest::new());
    c.submit_request(SwapRequest::new());

    let last_submitted = SwapRequest::last_id();

    let last_processed = c.process_requests();

    assert_eq!(last_processed, last_submitted);
}
