use super::*;
use std::fmt::{self, Display};

pub type Balances = (Amounts, Amount);
pub type UserStats = Vec<(User, Balances)>;
pub type UserAllows = Vec<(User, Amounts)>;

// A snapshot of some balances
#[derive(Debug, PartialEq)]
pub struct SwapStats {
    pub user_stats: UserStats,
    pub swap_pair: Balances,
    pub pair_allowances: UserAllows,
}

impl Display for SwapStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for s in &self.user_stats {
            let user = &s.0;
            let amounts = &s.1;
            write!(f, "\t{} {:?}\n", user.name, amounts)?;
        }
        write!(f, "\tPair  {:?}\n", self.swap_pair)?;
        for s in &self.pair_allowances {
            let user = &s.0;
            let amounts = &s.1;
            if amounts[0] > 0 || amounts[1] > 0 {
                write!(f, "\tPair allowance from {}: {:?}\n", user.name, amounts)?;
            }
        }
        Ok(())
    }
}
