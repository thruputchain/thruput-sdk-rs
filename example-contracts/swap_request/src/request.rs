#![allow(clippy::new_without_default)]

use sdk::*;
use sdk::__contract_spec::{
    ContractFieldView, ContractStructDefView, ContractTypeDefView,
    ContractTypeSpec, ContractTypeView,
};

pub type RequestID = u32;

/// A request to perform a swap.
#[derive(ReadXDR, WriteXDR)]
pub struct SwapRequest {
    pub id: RequestID,
    /* info about tokens to swap, amount, etc. */
}

impl ContractTypeSpec for SwapRequest {
    const TYPE_DEF: ContractTypeDefView<'static> =
        ContractTypeDefView::Struct(ContractStructDefView {
            name: "SwapRequest",
            fields: &[ContractFieldView {
                name: "id",
                ty: ContractTypeView::U32,
            }],
        });
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
