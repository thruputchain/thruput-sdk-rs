use thruput_erc721_if::{ERC721Receiver, TokenId, TransferData};
use thruput_sdk::storage::Value;
use thruput_sdk::*;

/// Attached data that makes the receiver reject the token.
pub const REJECT_DATA: &[u8] = b"reject";

#[thruput_sdk::storage]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct Receiver {
    last: Value<Delivery>,
}

/// What the hook saw on its most recent call.
#[contract_type]
#[cfg_attr(feature = "dev", derive(Debug, PartialEq, Eq))]
pub struct Delivery {
    pub operator: ContractAddress,
    pub from: ContractAddress,
    pub token_id: TokenId,
    pub data: TransferData,
}

#[contract_exception]
#[non_exhaustive]
pub enum ReceiverError {
    /// The delivery carried [`REJECT_DATA`].
    Refused = 1,
}

#[contract]
impl ERC721Receiver for Receiver {
    fn on_erc721_received(
        &mut self,
        operator: &ContractAddress,
        from: &ContractAddress,
        token_id: &TokenId,
        data: TransferData,
    ) {
        ensure(data.as_slice() != REJECT_DATA, ReceiverError::Refused);

        self.last.set(&Delivery {
            operator: *operator,
            from: *from,
            token_id: *token_id,
            data,
        });
    }
}

#[contract(client = ReceiverClient)]
impl Receiver {
    /// The most recent delivery, if any.
    pub fn last_delivery(&self) -> Option<Delivery> {
        self.last.try_get().none_if_missing()
    }
}
