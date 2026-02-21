use sdk::*;

#[derive(Default)]
pub struct HasherImpl;

#[contract(client = HasherClient, storage = "false")]
impl HasherImpl {
    pub fn run() {
        const EXPECTED: SHA256Hash =
            SHA256Hash::new(hash_to_array!("foo-bar-baz"));

        let mut h = Hasher::new();
        h.update(b"foo");
        h.update(b"-bar");
        h.update(b"-baz");

        let output = h.finish();

        if output != EXPECTED {
            panic!("Unexpected hash output");
        }
    }
}

#[cfg(feature = "dev")]
impl TestRegister for HasherImpl {
    fn register(env: &sdk::TestEnvironmentHandle) -> sdk::Contract {
        test_setup::register(env)
    }
}
