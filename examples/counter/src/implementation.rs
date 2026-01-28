use sdk::storage::NNInt;
use sdk::*;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum Error {
    #[error("Counter would go negative")]
    WouldGoNegative = 1,
    #[error("System error")]
    SysErr = 2,
}

impl From<SysErr> for Error {
    fn from(_: SysErr) -> Self {
        Error::SysErr
    }
}

impl From<sysresult::Error> for Error {
    fn from(err: sysresult::Error) -> Self {
        match err {
            sysresult::Error::NonNegativeIntUnderflow => {
                Error::WouldGoNegative
            }
            _ => Error::SysErr,
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;

/// A simple counter contract demonstrating commutative NNInt operations.
/// Multiple increment/decrement operations can execute in parallel and
/// merge correctly regardless of order.
#[derive(Debug)]
#[storage]
pub struct Counter {
    value: NNInt,
}

#[contract(exports = "guest_exports", err_type = Error, client = CounterClient)]
impl Counter {
    /// Initialize the counter with a starting value
    #[init]
    pub fn initialize(&mut self, initial_value: u64) -> Result<()> {
        self.value.checked_inc(initial_value as i128)?;
        Ok(())
    }

    /// Increment the counter by the given amount.
    /// This is a commutative operation - concurrent increments merge correctly.
    pub fn increment(&mut self, amount: u64) -> Result<()> {
        self.value.checked_inc(amount as i128)?;
        Ok(())
    }

    /// Decrement the counter by the given amount.
    /// Fails if the result would be negative.
    pub fn decrement(&mut self, amount: u64) -> Result<()> {
        self.value.checked_inc(-(amount as i128))?;
        Ok(())
    }

    /// Get the current counter value
    pub fn get(&self) -> Result<u64> {
        Ok(self.value.try_get()?.unwrap_or(0) as u64)
    }
}
