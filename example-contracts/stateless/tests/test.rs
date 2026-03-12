use stateless::*;

#[test]
fn test() {
    sdk::client_test(test_setup::register,
        |c: &mut StatelessClient| {
            c.run();
        });
}
