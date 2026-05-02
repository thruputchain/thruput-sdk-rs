use crate::mintable::MintableType;
use crate::{AuthTokenError, FaultString, xdr::*};
use core::marker::PhantomData;

pub struct Opaque(());

#[derive(Clone, Copy, Debug)]
pub struct Signed<T> {
    handle: u32,
    _phantom: PhantomData<T>,
}

impl<T> WriteXDR for Signed<T> {
    #[inline(always)]
    fn write_xdr<F, W: Writer<F>>(&self, _w: &mut W) -> WriteResult<()> {
        Ok(())
    }
}

impl<'r, T> ReadXDR<'r> for Signed<T> {
    #[inline(always)]
    fn read_xdr<R: Reader<'r>>(_r: &mut R) -> ReadResult<Self> {
        Ok(Self {
            handle: u32::MAX, // sentinel — must be filled by ReceiveAuthHandles
            _phantom: PhantomData,
        })
    }
}

impl<T> Signed<T> {
    #[cfg(feature = "alloc")]
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

/// Returns a Signed with sentinel handle (u32::MAX). Used internally
/// by Mintable to construct payload values — not for direct use.
impl<T> Default for Signed<T> {
    fn default() -> Self {
        Self {
            handle: u32::MAX,
            _phantom: PhantomData,
        }
    }
}

use crate::collect_auth_handles::{AuthHandleBuf, CollectAuthHandles};
use crate::receive_auth_handles::ReceiveAuthHandles;

impl<T> CollectAuthHandles for Signed<T> {
    #[inline(always)]
    fn collect_auth_handles(
        &self,
        out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString> {
        debug_assert_ne!(
            self.handle,
            u32::MAX,
            "Signed handle not initialized"
        );
        #[cfg(not(feature = "alloc"))]
        match out.push(self.handle) {
            Ok(()) => Ok(()),
            Err(e) => Err(FaultString::from_display(e)),
        }?;
        #[cfg(feature = "alloc")]
        out.push(self.handle);
        Ok(())
    }
}

impl<T> ReceiveAuthHandles for Signed<T> {
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError> {
        match handles.next() {
            Some(handle) => {
                self.handle = handle;
                Ok(())
            }
            None => Err(AuthTokenError::ReceiveAuthHandlesFailure(
                FaultString::from_display("missing auth handle for Signed"),
            )),
        }
    }
}

impl<T> groundhog_contract_spec::ContractTypeRefSpec for Signed<T> {
    const INFO: groundhog_contract_spec::ContractTypeInfo<'static> =
        groundhog_contract_spec::ContractTypeInfo {
            ty: groundhog_contract_spec::view::ContractTypeView::U32,
        };
}

#[cfg(feature = "alloc")]
use crate::mintable::{AuthTokenPreimage, Mintable};

impl<T: MintableType> MintableType for Signed<T> {
    type Preimage =
        crate::mintable::AuthTokenPreimage<<T as MintableType>::Preimage>;
}

#[cfg(feature = "alloc")]
impl<T: Mintable> Mintable for Signed<T>
where
    T::Preimage: crate::xdr::WriteXDR + for<'r> crate::xdr::ReadXDR<'r>,
    AuthTokenPreimage<T::Preimage>:
        crate::xdr::WriteXDR + for<'r> crate::xdr::ReadXDR<'r>,
{
    fn mint_into_parent(
        preimage: AuthTokenPreimage<<T as MintableType>::Preimage>,
        children: &mut crate::mintable::ChildNodeBuf,
    ) -> Self {
        let mut node =
            crate::mintable::build_auth_token_node::<T>(preimage.data);
        node.consumer_address = preimage.consumer;
        children.push(node).expect("too many auth token children");
        Signed::default()
    }
}

#[cfg(feature = "alloc")]
pub fn mint_auth_token<T: Mintable>(
    expiry: groundhog_xdr::types::BlockHeight,
    preimage: crate::mintable::AuthTokenPreimage<
        <T as MintableType>::Preimage,
    >,
) -> Signed<T>
where
    T::Preimage: crate::xdr::WriteXDR + for<'r> crate::xdr::ReadXDR<'r>,
    AuthTokenPreimage<T::Preimage>:
        crate::xdr::WriteXDR + for<'r> crate::xdr::ReadXDR<'r>,
{
    use crate::sys::SyscallNo;

    let mut root_node =
        crate::mintable::build_auth_token_node::<T>(preimage.data);
    root_node.consumer_address = preimage.consumer;
    let buf = crate::mintable::encode_auth_token_node(&root_node);

    let mut token_id: core::mem::MaybeUninit<u32> =
        core::mem::MaybeUninit::uninit();

    crate::sys::uninit_MintAuthToken(expiry, buf.as_ref(), &mut token_id)
        .unwrap_or_else(|e| {
            crate::exn::syscall_throw(SyscallNo::kMintAuthToken, e)
        });

    let handle = unsafe { token_id.assume_init() };
    Signed::from_handle(handle)
}

impl<T> Signed<T>
where
    T: for<'a> ReadXDR<'a> + crate::ReceiveAuthHandles,
{
    pub fn consume(self) -> T {
        debug_assert_ne!(
            self.handle,
            u32::MAX,
            "Signed handle not initialized"
        );

        use crate::internal::MUBuf;
        use crate::sys::SyscallNo;
        use core::mem::MaybeUninit;

        let mut payload_buf: MUBuf<500> = MUBuf::new();
        let mut payload_written: MaybeUninit<
            crate::types::RuntimeMemoryLength,
        > = MaybeUninit::uninit();
        let mut child_start: MaybeUninit<u32> = MaybeUninit::uninit();
        let mut child_count: MaybeUninit<u32> = MaybeUninit::uninit();

        crate::sys::uninit_ConsumeAuthToken(
            self.handle,
            payload_buf.spare_capacity_mut(),
            &mut payload_written,
            &mut child_start,
            &mut child_count,
        )
        .unwrap_or_else(|e| {
            crate::exn::syscall_throw(SyscallNo::kConsumeAuthToken, e)
        });

        let written = unsafe { payload_written.assume_init() } as usize;
        unsafe { payload_buf.set_len(written) };

        let start = unsafe { child_start.assume_init() };
        let count = unsafe { child_count.assume_init() };

        let mut value: T =
            cxs::decode(payload_buf.as_ref()).unwrap_or_else(|e| {
                crate::exn::library_throw(
                    crate::AuthTokenError::ConsumeDecodeFailure(
                        crate::exn::FaultString::from_display(e),
                    ),
                )
            });

        let end = start
            .checked_add(count)
            .expect("child handle range overflow");
        let mut iter = start..end;
        value
            .receive_auth_handles(&mut iter)
            .unwrap_or_else(|e| crate::exn::library_throw(e));

        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::MaybeUninit;

    #[test]
    fn signer_xdr_writes_zero_bytes() {
        let signer: Signed<u64> = Signed::from_handle(42);
        let mut buf: [MaybeUninit<u8>; 32] = [MaybeUninit::uninit(); 32];
        let written = cxs::encode_into(&signer, &mut buf).unwrap();
        assert_eq!(written, 0);
    }

    #[test]
    fn signer_xdr_reads_sentinel() {
        let decoded: Signed<u64> = cxs::decode(&[]).unwrap();
        assert_eq!(decoded.handle(), u32::MAX);
    }

    #[test]
    fn signer_xdr_in_tuple_excludes_handle() {
        let args: (u64, Signed<u32>, i32) = (100, Signed::from_handle(7), -1);

        let mut buf: [MaybeUninit<u8>; 64] = [MaybeUninit::uninit(); 64];
        let written = cxs::encode_into(&args, &mut buf).unwrap();

        let init: &[u8] = unsafe {
            core::slice::from_raw_parts(buf.as_ptr() as *const u8, written)
        };

        // Decode: Signer reads 0 bytes (gets sentinel), other fields normal
        let decoded: (u64, Signed<u32>, i32) = cxs::decode(init).unwrap();
        assert_eq!(decoded.0, 100);
        assert_eq!(decoded.1.handle(), u32::MAX); // sentinel, not 7
        assert_eq!(decoded.2, -1);
    }

    #[test]
    fn signer_receive_auth_handles() {
        use crate::collect_auth_handles::CollectAuthHandles;

        // Decode with sentinel handles
        let mut args: (u64, Signed<u32>, i32, Signed<bool>) = (
            100,
            Signed::from_handle(u32::MAX),
            -1,
            Signed::from_handle(u32::MAX),
        );

        // Fill in real handles
        let mut iter = [10u32, 20u32].into_iter();
        args.receive_auth_handles(&mut iter).unwrap();

        assert_eq!(args.1.handle(), 10);
        assert_eq!(args.3.handle(), 20);

        // Verify CollectAuthHandles works on the result
        let mut buf = crate::collect_auth_handles::AuthHandleBuf::new();
        args.collect_auth_handles(&mut buf).unwrap();
        assert_eq!(buf.as_slice(), &[10, 20]);
    }
}
