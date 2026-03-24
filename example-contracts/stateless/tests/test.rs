use stateless::*;

#[test]
fn test_run() {
    sdk::client_test(|env| env.register_mock::<Stateless, Stateless>(), |c: &mut StatelessClient| {
        c.run();
    });
}

use stateless::interface1;
use stateless::interface2;
use stateless::Stateless;

#[test]
fn test_ifaces() {
    sdk::client_test(
        |env| env.register_mock::<Stateless, dyn interface1::Interface>(),
        |c: &mut interface1::Client| {
            assert_eq!(c.perform_task(), 1);
        },
    );

    sdk::client_test(
        |env| env.register_mock::<Stateless, dyn interface2::Interface>(),
        |c: &mut interface2::Client| {
            assert_eq!(c.perform_task(), 2);
        },
    );
}
