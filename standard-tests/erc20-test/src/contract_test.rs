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

        let name = client_meta.name();
        let symbol = client_meta.symbol();

        assert_eq!(Some(TOKEN_NAME), name.as_ref().map(|n| n.as_str()));
        assert_eq!(Some(TOKEN_SYMBOL), symbol.as_ref().map(|s| s.as_str()));
        assert_eq!(Some(TOKEN_DECIMALS), client_meta.decimals());
    }

    #[cfg(not(feature = "metadata"))]
    client.initialize(&owner);

    // Mint (an amount != 100, so a hardcoded event amount can't hide)
    let amt = 101;
    client.mint(&owner, amt);
    m.mint(&owner, amt);

    // Transfer
    let amt = 25;
    client.transfer(other, amt);
    m.transfer(other, amt);

    #[cfg(not(feature = "no_check_aborts"))]
    {
        // Transfer too much
        assert!(m.balances[&owner] < 1000);
        assert_eq!(
            client.try_transfer(other, 1000).unwrap_err().try_as(),
            Some(ERC20Error::TooLarge),
        );
    }

    // Exercise positive, negative, and zero changes at the amount boundary.
    for difference in [i128::MAX, -(i128::MAX - 7), 10, -4, 0] {
        client.adjust_allowance(other, difference);
        m.adjust_allowance(other, difference);
        m.check_allowances(&client);
    }
    #[cfg(not(feature = "no_check_aborts"))]
    {
        assert_eq!(
            client
                .try_adjust_allowance(other, -14)
                .unwrap_err()
                .try_as(),
            Some(ERC20Error::TooLarge),
        );
        assert_eq!(
            client
                .try_adjust_allowance(other, i128::MAX)
                .unwrap_err()
                .try_as(),
            Some(ERC20Error::TooLarge),
        );
        assert_eq!(
            client.try_clear_balance().unwrap_err().try_as(),
            Some(ERC20Error::BalanceNotZero),
        );
    }
    for _ in 0..2 {
        client.clear_allowance(other);
        m.clear_allowance(other);
        m.check_allowances(&client);
    }

    // Check client state against the model
    m.check(&client);

    m
}

/// Test the transfer/allowance subset of the ERC20 logic, for a contract
/// that is already initialized and cannot mint (such as the native
/// token precompile).
///
/// The caller must already hold exactly `balance` tokens (at least 25, as
/// the test transfers a hardcoded 25 away), and the contract must report a
/// total supply of `total_supply`.
pub fn erc20_funded_test(
    deployed_addr: &ContractAddress,
    balance: Amount,
    total_supply: Amount,
) -> Model {
    let owner = get_current_address();
    let other = &OTHER;

    let mut m = Model::default();
    m.initialize(&owner);
    m.total_supply = total_supply;
    m.balances.insert(owner, balance);

    let client = ERC20Client::new(deployed_addr);

    // Transfer
    let amt = 25;
    assert!(m.balances[&owner] >= amt);
    client.transfer(other, amt);
    m.transfer(other, amt);

    client.adjust_allowance(other, 17);
    m.adjust_allowance(other, 17);
    client.adjust_allowance(other, -10);
    m.adjust_allowance(other, -10);

    // Check client state against the model
    m.check(&client);

    m
}
