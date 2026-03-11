use sdk::*;
use swap_request::*;

client_test! { SwapContract, SwapTest =>
    fn mytest(c: &mut SwapClient) {
        c.submit_request(SwapRequest::new());
        c.submit_request(SwapRequest::new());
        c.submit_request(SwapRequest::new());
        c.submit_request(SwapRequest::new());

        let last_submitted = SwapRequest::last_id();

        let last_processed = c.process_requests();

        assert_eq!(last_processed, last_submitted);
    }
}
