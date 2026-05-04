use crate::model::Model;
use groundhog_erc20_if::*;
use sdk::*;

#[cfg(feature = "metadata")]
mod metadata {
    pub use groundhog_erc20_metadata_if::*;

    pub const TOKEN_NAME: &str = "Test Token";
    pub const TOKEN_SYMBOL: &str = "TEST-TOK";
    pub const TOKEN_DECIMALS: u32 = 18;
}
#[cfg(feature = "metadata")]
use metadata::*;

hash_ident! { static TEST_NONCE: SHA256Hash;  }
hash_ident! { static OTHER: ContractAddress; }

pub fn erc20_test_setup(target_contract_hash: &SHA256Hash) -> ContractAddress {
    // Deploy the target contract
    deploy_contract(&TEST_NONCE, target_contract_hash)
}

/// Test the logic of an ERC20 contract
pub fn erc20_test(deployed_addr: &ContractAddress) -> Model {
    let owner = get_current_address();
    let other = &OTHER;

    let mut m = Model::default();
    m.initialize(&owner);

    let client = ERC20Client::new(deployed_addr);

    #[cfg(feature = "metadata")]
    {
        let client_meta = ERC20MetadataClient::new(deployed_addr);
        client_meta.initialize(
            &owner,
            Some(TOKEN_NAME.try_into().unwrap()),
            Some(TOKEN_SYMBOL.try_into().unwrap()),
            Some(TOKEN_DECIMALS),
        );

        assert_eq!(
            Some(TOKEN_NAME.to_string()),
            client_meta.name().map(|s| s.into()),
        );
        assert_eq!(
            Some(TOKEN_SYMBOL.to_string()),
            client_meta.symbol().map(|s| s.into()),
        );
        assert_eq!(Some(TOKEN_DECIMALS), client_meta.decimals());
    }

    #[cfg(not(feature = "metadata"))]
    client.initialize(&owner);

    // Mint
    let amt = 100;
    client.mint(&owner, amt);
    m.mint(&owner, amt);

    // Burn
    let amt = 50;
    client.burn(&owner, amt);
    m.burn(&owner, amt);

    #[cfg(not(feature = "no_check_aborts"))]
    {
        // Burn too much
        let amt = 60;
        assert!(m.balances[&owner] < amt);
        assert!(matches!(
            client.burn(&owner, amt),
            Err(ClientError::Aborted(_ /* ERC20Error::TooLarge */))
        ));
    }

    // Transfer
    let amt = 25;
    client.transfer(other, amt);
    m.transfer(other, amt);

    #[cfg(not(feature = "no_check_aborts"))]
    {
        // Transfer too much
        assert!(m.balances[&owner] < 1000);
        assert!(matches!(
            client.transfer(other, 1000),
            Err(ClientError::Aborted(_ /* ERC20Error::TooLarge */))
        ));
    }

    // Burn from other address
    let amt = 5;
    client.burn(other, amt);
    m.burn(other, amt);

    // Approve spending by "other"
    let amt = 17;
    client.adjust_allowance(other, amt);
    m.adjust_allowance(other, amt);

    #[cfg(not(feature = "no_check_aborts"))]
    // Withdraw too much approval
    assert!(matches!(
        client.adjust_allowance(other, -20),
        Err(ClientError::Aborted(_)),
    ));

    // Withdraw some approval
    let amt = 10;
    client.adjust_allowance(other, -amt);
    m.adjust_allowance(other, -amt);

    // Check client state against the model
    m.check(&client);

    m
}
