use sdk::storage::*;
use sdk::*;

/// Limit can be reset once more than `period_frequency` blocks have passed
/// since the last reset.
///
/// Epoch-based limits would be a small change to this implementation.
#[derive(WriteXDR, ReadXDR)]
#[xdrs(crate = "sdk::__xdrs")]
struct WithdrawalConfiguration {
    per_period_limit: i128,
    period_frequency: BlockHeight,
    last_reset_height: BlockHeight,
}

#[storage]
struct VelocityLimit {
    configuration: Value<WithdrawalConfiguration>,
    remaining_capacity: CapacityConstraint,
}

#[derive(ReadXDR, WriteXDR)]
#[xdrs(crate = "sdk::__xdrs")]
struct VelocityLimitConfiguration {
    token: ContractAddress,
    owner: ContractAddress,
}

#[storage]
pub struct VelocityLimitedWithdrawal {
    config: Value<VelocityLimitConfiguration>,
    limits: Map<ContractAddress, Node<VelocityLimit>>,
}

#[contract(client = VelocityLimitedWithdrawalClient)]
impl VelocityLimitedWithdrawal {
    pub fn initialize(
        &mut self,
        owner: ContractAddress,
        token: ContractAddress,
    ) {
        self.config
            .set(&VelocityLimitConfiguration { token, owner });
    }

    pub fn set_limit(
        &mut self,
        receiver: ContractAddress,
        limit: i128,
        period_frequency: BlockHeight,
    ) {
        assert!(
            get_caller() == self.config.get().owner,
            "only the owner may set limits"
        );
        let limit = WithdrawalConfiguration {
            per_period_limit: limit,
            period_frequency,
            last_reset_height: get_block_height(),
        };
        let mut new_limit = self.limits.at_mut(&receiver);
        new_limit.configuration.set(&limit);
        new_limit
            .remaining_capacity
            .set_capacity(limit.per_period_limit);
    }

    pub fn withdraw(&mut self, amount: i128) {
        assert!(amount >= 0, "amount must be non-negative");
        let receiver = get_caller();
        let mut limit = self.limits.at_mut(&receiver);
        let mut config = limit.configuration.get();
        // if the limit's period has elapsed, reset it
        let current_height = get_block_height();
        if &current_height - &config.last_reset_height
            > *config.period_frequency
        {
            config.last_reset_height = current_height;
            limit
                .remaining_capacity
                .set_capacity(config.per_period_limit);
            limit.configuration.set(&config);
        }

        limit.remaining_capacity.consume_capacity(amount);

        let token =
            thruput_erc20_if::ERC20Client::new(&self.config.get().token);
        token.transfer(&receiver, amount);
    }
}
