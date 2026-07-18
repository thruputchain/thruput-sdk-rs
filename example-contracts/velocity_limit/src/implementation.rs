use sdk::*;
use sdk::storage::*;


/// Limit can be reset when at least the configured number of blocks has passed
/// since the last reset.
///
/// Epoch-based limits would be a small change to this implementation.
#[derive(WriteXDR,ReadXDR)]
struct WithdrawalConfiguration {
    per_period_limit: i128,
    period_frequency: BlockHeight,
    last_reset_height: BlockHeight,
}

#[derive(Storage)]
struct VelocityLimit {
    configuration: Value<WithdrawalConfiguration>,
    remaining_capacity: LinearConstraint,
}

#[derive(ReadXDR, WriteXDR)]
struct VelocityLimitConfiguration {
    token: ContractAddress,
    owner: ContractAddress,
}

#[derive(Storage)]
pub struct VelocityLimitedWithdrawal {
    config: Value<VelocityLimitConfiguration>,
    limits: Map<ContractAddress, Node<VelocityLimit>>,
}

#[contract(client = VelocityLimitedWithdrawalClient)]
impl VelocityLimitedWithdrawal {
    pub fn initialize(&mut self, owner: ContractAddress, token: ContractAddress) {
        self.config.set(&VelocityLimitConfiguration {
            token,
            owner,
        });
    }

    pub fn set_limit(&mut self, receiver: ContractAddress, limit: i128, period_frequency: BlockHeight) {
        let limit = WithdrawalConfiguration {
            per_period_limit: limit,
            period_frequency,
            last_reset_height: get_block_height(),
        };
        let mut new_limit = self.limits.at_mut(&receiver);
        new_limit.configuration.set(&limit);
        new_limit.remaining_capacity.set_capacity(limit.per_period_limit);
    }

    pub fn withdraw(&mut self, amount: i128) {
        assert!(amount >= 0, "amount must be positive");
        let receiver = get_caller();
        let mut limit = self.limits.at_mut(&receiver);
        let mut config = limit.configuration.get();
        // if limit expired, reset it
        let current_height = get_block_height();
        if *config.last_reset_height < *current_height - *config.period_frequency {
            config.last_reset_height = current_height;
            limit.remaining_capacity.set_capacity(config.per_period_limit);
            limit.configuration.set(&config);
        }

        limit.remaining_capacity.apply_delta(amount);

        let token : groundhog_erc20_if::ERC20Client = groundhog_erc20_if::ERC20Client::new(&self.config.get().token);
        token.transfer(&receiver, amount as u128);
    }
}
