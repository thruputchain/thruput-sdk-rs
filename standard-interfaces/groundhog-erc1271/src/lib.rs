#![cfg_attr(not(feature = "std"), no_std)]

pub type ERC1271Error = ::sdk::contract_result::Error;

use sdk::contract;

#[contract(client = ERC1271Client, err_type = ERC1271Error)]
pub trait ERC1271 {
    /// Verifies if the given signature is valid for the given message hash.
    ///
    /// Note: Unlike the ERC1271 standard, the msg here is a &[u8], not a &[u8;32].
    /// A signature scheme might require some metadata on the message (such as which public key
    /// it refers to) and we want the standard to be agnostic to that metadata.
    /// We also want to allow the is_valid_signature to be able to operate on the message contents
    /// if necessary, instead of just on the message hash.
    fn is_valid_signature(
        &self,
        message: &[u8],
        signature: &[u8],
    ) -> Result<bool, ERC1271Error>;
}
