use sdk::{ContractAddress, EmittedEvent, Event, log_event};

/// An absolute SEP-41 grant. The expiry is a Groundhog block height.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct SEP41ApprovalEvent<'a> {
    #[topic]
    pub from: &'a ContractAddress,
    #[topic]
    pub spender: &'a ContractAddress,
    pub amount: i128,
    pub live_until_ledger: u32,
}

impl<'a> SEP41ApprovalEvent<'a> {
    pub fn new(
        from: &'a ContractAddress,
        spender: &'a ContractAddress,
        amount: i128,
        live_until_ledger: u32,
    ) -> Self {
        Self {
            from,
            spender,
            amount,
            live_until_ledger,
        }
    }
    pub fn log(&self) {
        log_event(self);
    }
}

impl<'a> TryFrom<EmittedEvent<'a>> for SEP41ApprovalEvent<'a> {
    type Error = &'static str;
    fn try_from(event: EmittedEvent<'a>) -> Result<Self, Self::Error> {
        if event.topic(0).ok_or("No event signature")? != Self::SIGNATURE {
            return Err("Unexpected event signature");
        }
        let from = event.topic(1).ok_or("Missing from topic")?.into();
        let spender = event.topic(2).ok_or("Missing spender topic")?.into();
        let payload = event
            .payload()
            .ok_or("Malformed event: topics overrun body")?;
        let (amount, live_until_ledger) =
            sdk::internal::decode_xdr(payload)
                .map_err(|_| "Couldn't parse event payload")?;
        Ok(Self::new(from, spender, amount, live_until_ledger))
    }
}
