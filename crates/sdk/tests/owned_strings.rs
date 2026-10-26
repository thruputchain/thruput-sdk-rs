#![allow(unexpected_cfgs)]

use thruput_sdk::*;

pub struct OwnedStrings;

#[contract]
impl OwnedStrings {
    pub fn echo(text: String) -> String {
        text
    }

    pub fn optional(text: Option<String>) -> Option<String> {
        text
    }
}

#[test]
fn owned_strings_round_trip_through_generated_clients() {
    let env = TestEnvironment::no_cost();
    let target = ContractAddress::named("owned-strings");
    env.register_mock(OwnedStrings::__mock)
        .deploy_contract_to(&target);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("caller")).run(|| {
            let client = OwnedStringsClient::new(&target);
            for text in ["", "hello", "Unicode: 🦀"] {
                assert_eq!(client.echo(text.to_owned()), text);
                assert_eq!(
                    client.optional(Some(text.to_owned())),
                    Some(text.to_owned())
                );
            }
            assert_eq!(client.optional(None), None);
        })
    })
    .assert_success();
}
