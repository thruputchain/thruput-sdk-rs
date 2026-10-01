//! Runs the same HostFunction calldata through soroban-env-host natively
//! against an in-memory snapshot: the reference the on-chain results must match.
use soroban_env_host::{budget::Budget, e2e_invoke::*, storage::*, xdr::*, HostError, LedgerInfo};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
struct Mem(RefCell<BTreeMap<Vec<u8>, (Vec<u8>, Option<u32>)>>);
impl SnapshotSource for Mem {
    fn get(&self, key: &Rc<LedgerKey>) -> Result<Option<EntryWithLiveUntil>, HostError> {
        let k = key.to_xdr(Limits::none())?;
        Ok(self.0.borrow().get(&k).map(|(v, t)| (Rc::new(LedgerEntry::from_xdr(v, Limits::none()).unwrap()), *t)))
    }
}
fn main() {
    let mem = Rc::new(Mem(RefCell::new(BTreeMap::new())));
    for f in std::env::args().skip(1) {
        let hx = std::fs::read_to_string(&f).unwrap();
        let hf = HostFunction::from_xdr(hex::decode(hx.trim().trim_start_matches("0x")).unwrap(), Limits::none()).unwrap();
        let li = LedgerInfo { protocol_version: soroban_env_host::meta::INTERFACE_VERSION.protocol, sequence_number: 1000, timestamp: 0, network_id: [7; 32], base_reserve: 1, min_temp_entry_ttl: 16, min_persistent_entry_ttl: 4096, max_entry_ttl: 6_312_000 };
        let src = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256([1; 32])));
        let mut diag = vec![];
        let r = invoke_host_function_in_recording_mode(&Budget::default(), true, &hf, &src,
            RecordingInvocationAuthMode::Recording(RecordingInvocationAuthParams::new(false, false)),
            li, mem.clone(), [0; 32], &mut diag).unwrap();
        println!("{f}: {:?}", r.invoke_result.as_ref().map_err(|e| e.error));
        if r.invoke_result.is_err() { for d in &diag { println!("  diag {:?}", d.event.body); } }
        for ch in r.ledger_changes.iter().filter(|c| !c.read_only) {
            let ttl = ch.ttl_change.as_ref().map(|t| t.new_live_until_ledger);
            match &ch.encoded_new_value { Some(v) => { mem.0.borrow_mut().insert(ch.encoded_key.clone(), (v.clone(), ttl)); } None => { mem.0.borrow_mut().remove(&ch.encoded_key); } }
        }
    }
}
