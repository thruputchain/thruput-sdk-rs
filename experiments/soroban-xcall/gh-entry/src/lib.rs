//! The trusted Soroban entrypoint.
//!
//! Method 0, raw calldata:
//!   target: [u8; 32]                         Soroban contract (= Groundhog address)
//!   hf_len: u32 BE, hf: [u8; hf_len]         XDR HostFunction::InvokeContract
//!   n: u32 BE, then n x (len: u32 BE, node)  txs-encoded AuthTokenNode trees
//!
//! A real entrypoint verifies the Soroban signatures over each tree and
//! sequences nonces (RCQ) before minting; this experiment assumes both are
//! done. Minting makes the trees unforgeable: the runtime stamps this
//! contract's address on every token, and the Soroban-hosting contracts only
//! accept tokens minted here.
#![cfg_attr(not(feature = "dev"), no_std)]

extern crate alloc;

#[cfg(target_arch = "riscv32")]
mod rv32_entries {
    use alloc::vec::Vec;
    use sdk::__rv32::Entrypoint;
    use sdk::call::{CallOutcome, contract_call};
    use sdk::contract::ContractEntrypointID;
    use sdk::types::ContractAddress;

    fn take<'a>(b: &mut &'a [u8], n: usize) -> &'a [u8] {
        if b.len() < n {
            sdk::contract_op::contract_revert();
        }
        let (h, t) = b.split_at(n);
        *b = t;
        h
    }

    fn take_u32(b: &mut &[u8]) -> usize {
        u32::from_be_bytes(take(b, 4).try_into().unwrap()) as usize
    }

    extern "C" fn invoke() {
        let mut calldata = sdk::internal::new_scratch();
        let mut ids = sdk::contract_op::AuthTokenIdBuf::new();
        sdk::contract_op::fetch_calldata_and_auth_tokens(&mut calldata, &mut ids);
        let mut b: &[u8] = calldata.as_ref();
        let target: [u8; 32] = take(&mut b, 32).try_into().unwrap();
        let hf_len = take_u32(&mut b);
        let hf = take(&mut b, hf_len);
        let n = take_u32(&mut b);
        let expiry = sdk::block::get_block_height();
        let mut tokens = Vec::new();
        for _ in 0..n {
            let len = take_u32(&mut b);
            let node = take(&mut b, len);
            tokens.push(sdk::signer::mint_opaque_auth_token(expiry.clone(), node).handle());
        }
        let mut ret = Vec::new();
        let mut ret_auth = Vec::new();
        match contract_call(
            &ContractAddress::from(target),
            ContractEntrypointID::from_u64(0),
            u64::MAX,
            hf,
            &tokens,
            Default::default(),
            &mut ret,
            &mut ret_auth,
        ) {
            Ok(CallOutcome::Return) => sdk::sys::raw_Return(&ret, false, &[]),
            Ok(CallOutcome::Abort) => sdk::contract_op::contract_abort(&ret),
            Err(_) => sdk::contract_op::contract_revert(),
        }
    }

    #[used]
    #[unsafe(link_section = "groundhog_rv32_entrypoints")]
    static INVOKE: Entrypoint = Entrypoint { method_id: 0, wrapper: invoke };
}
