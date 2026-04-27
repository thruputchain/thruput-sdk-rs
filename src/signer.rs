use crate::call::internal::contract_call_encode;
use crate::exn::{FaultString, library_throw, syscall_throw};
use crate::sys::{self, SyscallNo};
use crate::{AuthTokenError, xdr::*};
use core::marker::PhantomData;
use groundhog_xdr::types::BlockHeight;

#[derive(Clone, Copy, ReadXDR, WriteXDR)]
#[xdrs(crate = "crate::__xdrs")]
pub struct Signer<T> {
    handle: u32,
    _phantom: PhantomData<T>,
}

impl<T> Signer<T> {
    pub(crate) fn from_handle(handle: u32) -> Self {
        Self {
            handle,
            _phantom: PhantomData,
        }
    }

    pub fn handle(&self) -> u32 {
        self.handle
    }
}

impl<T> core::fmt::Debug for Signer<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Signer")
            .field("handle", &self.handle)
            .finish()
    }
}

use crate::collect_auth_handles::{AuthHandleBuf, CollectAuthHandles};

impl<T> CollectAuthHandles for Signer<T> {
    #[inline(always)]
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf) {
        out.push(self.handle).expect("auth handle buffer overflow");
    }
}

impl<T> groundhog_contract_spec::ContractTypeRefSpec for Signer<T> {
    const INFO: groundhog_contract_spec::ContractTypeInfo<'static> =
        groundhog_contract_spec::ContractTypeInfo {
            ty: groundhog_contract_spec::view::ContractTypeView::U32,
        };
}

pub fn mint_auth_token<T: WriteXDR>(
    expiry: BlockHeight,
    data: &T,
) -> Signer<T> {
    let buf = contract_call_encode(data).unwrap_or_else(|e| {
        library_throw(AuthTokenError::MintEncodeFailure(
            FaultString::from_display(e),
        ))
    });

    let mut token_id: core::mem::MaybeUninit<u32> =
        core::mem::MaybeUninit::uninit();

    sys::uninit_MintAuthToken(expiry, buf.as_ref(), &mut token_id)
        .unwrap_or_else(|e| syscall_throw(SyscallNo::kMintAuthToken, e));

    let handle = unsafe { token_id.assume_init() };
    Signer::from_handle(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::MaybeUninit;

    #[test]
    fn signer_xdr_roundtrip() {
        let signer: Signer<u64> = Signer::from_handle(42);

        let mut buf: [MaybeUninit<u8>; 32] = [MaybeUninit::uninit(); 32];
        let written = cxs::encode_into(&signer, &mut buf).unwrap();
        assert!(written > 0);

        // SAFETY: encode_into initialized `written` bytes.
        let init: &[u8] = unsafe {
            core::slice::from_raw_parts(buf.as_ptr() as *const u8, written)
        };
        let decoded: Signer<u64> = cxs::decode(init).unwrap();
        assert_eq!(decoded.handle(), 42);
    }

    #[test]
    fn signer_xdr_as_tuple_member() {
        let args: (u64, Signer<u32>, i32) = (100, Signer::from_handle(7), -1);

        let mut buf: [MaybeUninit<u8>; 64] = [MaybeUninit::uninit(); 64];
        let written = cxs::encode_into(&args, &mut buf).unwrap();

        // SAFETY: encode_into initialized `written` bytes.
        let init: &[u8] = unsafe {
            core::slice::from_raw_parts(buf.as_ptr() as *const u8, written)
        };
        let decoded: (u64, Signer<u32>, i32) = cxs::decode(init).unwrap();
        assert_eq!(decoded.0, 100);
        assert_eq!(decoded.1.handle(), 7);
        assert_eq!(decoded.2, -1);
    }
}
