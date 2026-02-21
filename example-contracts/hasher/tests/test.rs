sdk::client_test! { hasher::HasherImpl =>

    fn hasher_test(c: &mut hasher::HasherClient) {
        c.run();
    }
}
