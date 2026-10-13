use thruput_sdk::*;

static KEY: StorageKey = static_key!("invoke_flags_test_key");

pub struct KeyValueStore;

#[contract(client = KeyValueStoreClient)]
impl KeyValueStore {
    pub fn write(value: u64) {
        bytestring_set(&KEY, &value.to_be_bytes());
    }
}
