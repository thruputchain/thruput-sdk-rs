use stateless::*;

#[test]
fn test_run() {
    sdk::client_test(test_setup::register, |c: &mut StatelessClient| {
        c.run();
    });
}

use stateless::interface1;
use stateless::interface2;

#[test]
fn test_ifaces() {
    sdk::client_test(
        interface1::test_setup::register,
        |c: &mut interface1::Client| {
            assert_eq!(c.perform_task(), 1);
        },
    );

    sdk::client_test(
        interface2::test_setup::register,
        |c: &mut interface2::Client| {
            assert_eq!(c.perform_task(), 2);
        },
    );
}
