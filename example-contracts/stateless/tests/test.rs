use stateless::*;

#[test]
fn test_run() {
    sdk::client_test(|env| env.register_mock(Stateless::__mock), |c: &mut StatelessClient| {
        c.run();
    });
}

use stateless::interface1;
use stateless::interface2;
use stateless::Stateless;

#[test]
fn test_ifaces() {
    sdk::client_test(
        |env| env.register_mock(<Stateless as interface1::Interface>::__mock),
        |c: &mut interface1::Client| {
            assert_eq!(c.perform_task(), 1);
        },
    );

    sdk::client_test(
        |env| env.register_mock(<Stateless as interface2::Interface>::__mock),
        |c: &mut interface2::Client| {
            assert_eq!(c.perform_task(), 2);
        },
    );
}
