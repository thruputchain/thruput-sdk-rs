use crate::xdr::*;
use core::marker::PhantomData;

#[derive(Clone, Copy)]
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

impl<T> WriteXDR for Signer<T> {
    #[inline(always)]
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        self.handle.write_xdr(w)
    }
}

impl<'r, T> ReadXDR<'r> for Signer<T> {
    #[inline(always)]
    fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
        let handle = u32::read_xdr(r)?;
        Ok(Self::from_handle(handle))
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
