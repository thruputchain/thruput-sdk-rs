#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::new_without_default)]
#![allow(dead_code)]

use sdk::*;
use sdk::storage::RCQ;

static REQUEST_QUEUE: StorageKey = static_key!("queue key");

pub type RequestID = u32;

#[derive(ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
pub struct SwapRequest {
    pub id: RequestID,
    /* info about tokens to swap, amount, etc. */
}

contract_local! {
    static LAST_REQUEST_ID: ContractCell<RequestID> = ContractCell::new(0);
}

impl SwapRequest {
    pub fn new() -> Self {
        let id = LAST_REQUEST_ID.get() + 1;
        LAST_REQUEST_ID.set(id);

        Self { id }
    }

    pub fn last_id() -> RequestID {
        LAST_REQUEST_ID.get()
    }
}

type EncodedSwapRequest = [u8; 4];

impl From<EncodedSwapRequest> for SwapRequest {
    fn from(array: EncodedSwapRequest) -> Self {
        let id: RequestID = u32::from_be_bytes(array);
        SwapRequest { id }
    }
}

impl From<SwapRequest> for EncodedSwapRequest {
    fn from(s: SwapRequest) -> Self {
        s.id.to_be_bytes()
    }
}

#[storage]
pub struct SwapContract {
    requests: RCQ,
}

#[contract(client = SwapClient)]
impl SwapContract {
    pub fn submit_request(&mut self, r: SwapRequest) {
        let r: EncodedSwapRequest = r.into();

        self.requests.insert(&r);
    }

    pub fn process_requests(&mut self) -> RequestID {
        let mut extent = 0;

        while let Some(r) = self.dequeue_request() {
            // Keep track of the highest request id
            extent = extent.max(r.id);
        }

        extent
    }
}

impl SwapContract {
    fn dequeue_request(&mut self) -> Option<SwapRequest> {

        // Consume a queue entry
        let req: EncodedSwapRequest = self.requests.try_consume_sized()?;

        Some(req.into())
    }
}
