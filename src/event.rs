use crate::sysresult::Result;
use crate::sys;
use crate::WriteXDR;
use crate::internal::encode_xdr;
use crate::hash::*;

pub const EVENT_MAX_TOPICS: usize = 3;

pub trait Event {
    type Data: WriteXDR;

    /// Implement this method if you can provide a reference to a hashable value
    fn topic_by_ref(&self, index: u32) -> Option<&dyn Hash> {
        let _ = index;

        None
    }

    /// Implement this method to provide an owned hash value
    fn topic(&self, index: u32) -> Option<Hash256> {
        self.topic_by_ref(index).map(Hash::hash)
    }

    fn data(&self) -> Self::Data;
}

pub fn log_event(event: &impl Event) -> Result<()> {
    let topic0 = event.topic(0);
    let topic1 = event.topic(1);
    let topic2 = event.topic(2);

    let data = event.data();
    let data = encode_xdr(&data)?;

    sys::uninit_LogEvent(
        topic0.as_ref(),
        topic1.as_ref(),
        topic2.as_ref(),
        data.as_ref()
    )?;

    Ok(())
}
