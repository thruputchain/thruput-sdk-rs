use crate::WriteXDR;
use crate::internal::encode_xdr;
use crate::sys;
use crate::sysresult::Result;
use crate::types::SHA256Hash;

/*
pub struct EventData {
    pub topics: VecM<SHA256Hash, 3>,
    pub event: BytesM,
}
*/

pub const EVENT_MAX_TOPICS: usize = 3;

pub trait Event {
    fn topics(&self) -> [Option<&SHA256Hash>; EVENT_MAX_TOPICS];
    fn data(&self) -> impl WriteXDR;
}

pub fn log_event(event: &impl Event) -> Result<()> {
    let topics = event.topics();
    let topic0 = topics[0];
    let topic1 = topics[1];
    let topic2 = topics[2];

    let data = event.data();
    let data = encode_xdr(&data)?;

    sys::uninit_LogEvent(topic0, topic1, topic2, data.as_ref())?;

    Ok(())
}
