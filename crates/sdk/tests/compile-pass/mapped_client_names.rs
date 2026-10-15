#![allow(unexpected_cfgs)]
use thruput_sdk::*;

#[contract]
pub trait Names {
    fn foo();
    fn foo_with();
    fn map_exceptions();
    fn r#type();
}

// A caller's type with the adapter's internal name must remain valid.
pub struct ExceptionMappedClient;

fn check(client: NamesClient) {
    let _ = MapExceptions::map_exceptions(&client, |_| ()).try_foo();
    let _ = MapExceptions::map_exceptions(&client, |_| ()).try_foo_with();
    let _ = MapExceptions::map_exceptions(&client, |_| ()).try_map_exceptions();
    let _ = MapExceptions::map_exceptions(&client, |_| ()).try_type();
}

fn main() { let _ = check; }

mod explicit_client {
    use thruput_sdk::*;

    type __ThruputException = u32;
    type __ThruputExceptionHandler = u32;
    type __ThruputMappedClient = u32;

    #[contract(client = F)]
    pub trait Custom {
        fn call(value: __ThruputExceptionHandler, other: __ThruputMappedClient) -> __ThruputException;
    }

    pub fn check(client: F) {
        let _: Result<u32, thruput_sdk::call::RawCallError<u8>> =
            client.map_exceptions(|_| 1u8).try_call(2, 3);
    }
}

mod raw_client {
    use thruput_sdk::*;

    #[contract(client = r#__ThruputMappedClient)]
    pub trait RawClient {
        fn call();
    }

    pub fn check(client: r#__ThruputMappedClient) {
        let _ = client.map_exceptions(|_| ()).try_call();
    }
}
