use groundhog_xdr::types::ContractAddress;

use crate::mintable::MintableType;
use crate::{AuthTokenError, FaultString, xdr::*};
use core::marker::PhantomData;

/// Type-erased marker for [`Signed`] auth tokens. A `Signed<Opaque>` carries a
/// valid auth token handle but hides the payload type. Use [`Signed::assume`] to
/// reinterpret as a concrete `Signed<T>`, or [`AssumeTokens`] to destructure a
/// `Vec<Signed<Opaque>>` into a typed tuple.
#[derive(Debug)]
pub struct Opaque(());

#[derive(Debug)]
pub struct Signed<T> {
    handle: u32,
    _phantom: PhantomData<T>,
}

impl<T> WriteXDR for Signed<T> {
    #[inline(always)]
    fn write_xdr<F, W: Writer<F>>(&self, _w: &mut W) -> WriteResult<()> {
        Ok(())
    }

    #[inline(always)]
    unsafe fn write_xdr_unchecked<F, W: WriterUnchecked<F>>(
        &self,
        _w: &mut W,
    ) {
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

    /// An uninitialized `Signed` carrying the sentinel handle (`u32::MAX`).
    ///
    /// The handle must be filled via [`ReceiveAuthHandles`] before the value is
    /// used; collecting or consuming it while uninitialized aborts. This is the
    /// same value a freshly-decoded `Signed` holds before its handle arrives
    /// out-of-band.
    ///
    /// Intentionally an explicit, `#[doc(hidden)]` constructor rather than a
    /// `Default` impl: a sentinel must not be reachable via `#[derive(Default)]`
    /// on a containing struct, which would silently leave an auth field
    /// uninitialized.
    #[doc(hidden)]
    pub fn uninitialized() -> Self {
        Self {
            handle: u32::MAX,
            _phantom: PhantomData,
        }
    }

    pub fn handle(&self) -> u32 {
        self.handle
    }
}

impl Signed<Opaque> {
    pub fn assume<T: MintableType>(self) -> Signed<T> {
        Signed {
            handle: self.handle,
            _phantom: PhantomData,
        }
    }
}

#[cfg(feature = "alloc")]
pub trait AssumeTokens: Sized {
    /// Destructure the auth tokens returned by a called contract into a
    /// typed tuple, checking the count.
    ///
    /// The callee decides how many tokens it returns, so a wrong count is
    /// untrusted input, not a caller bug: it is reported as
    /// [`CallingError::DecodingSuccess`](crate::exn::CallingError) — the same
    /// cross-call error as a return-payload decode failure. Propagate it as a
    /// typed abort with
    /// [`or_re_throw`](crate::re_throw::OrReThrow::or_re_throw).
    fn try_assume_tokens(
        tokens: alloc::vec::Vec<Signed<Opaque>>,
    ) -> Result<Self, crate::exn::CallingError>;

    /// Throwing convenience form:
    /// [`try_assume_tokens`](Self::try_assume_tokens) with a count mismatch
    /// propagated as a typed abort.
    #[cfg_attr(not(target_arch = "riscv32"), track_caller)]
    fn assume_tokens(tokens: alloc::vec::Vec<Signed<Opaque>>) -> Self {
        use crate::re_throw::OrReThrow;
        Self::try_assume_tokens(tokens).or_re_throw()
    }
}

/// Report a returned-token count mismatch as the cross-call decode error.
#[cfg(feature = "alloc")]
fn check_token_count(
    got: usize,
    expected: usize,
) -> Result<(), crate::exn::CallingError> {
    if got == expected {
        Ok(())
    } else {
        Err(crate::exn::CallingError::DecodingSuccess(
            FaultString::from_display(format_args!(
                "expected {expected} auth tokens, got {got}"
            )),
        ))
    }
}

#[cfg(feature = "alloc")]
macro_rules! impl_assume_tokens {
    () => {
        impl AssumeTokens for () {
            fn try_assume_tokens(
                tokens: alloc::vec::Vec<Signed<Opaque>>,
            ) -> Result<Self, crate::exn::CallingError> {
                check_token_count(tokens.len(), 0)
            }
        }
    };
    ($($idx:tt : $T:ident),+) => {
        impl<$($T: MintableType),+> AssumeTokens for ($(Signed<$T>,)+) {
            fn try_assume_tokens(
                tokens: alloc::vec::Vec<Signed<Opaque>>,
            ) -> Result<Self, crate::exn::CallingError> {
                const N: usize = impl_assume_tokens!(@count $($T),+);
                check_token_count(tokens.len(), N)?;
                let mut iter = tokens.into_iter();
                Ok(($(impl_assume_tokens!(@take iter, $T),)+))
            }
        }
    };
    (@take $iter:ident, $T:ident) => {
        $iter.next().unwrap().assume::<$T>()
    };
    (@count $head:ident $(, $tail:ident)*) => {
        1 + impl_assume_tokens!(@count $($tail),*)
    };
    (@count) => { 0 };
}

#[cfg(feature = "alloc")]
impl_assume_tokens!();
#[cfg(feature = "alloc")]
impl_assume_tokens!(0: A);
#[cfg(feature = "alloc")]
impl_assume_tokens!(0: A, 1: B);
#[cfg(feature = "alloc")]
impl_assume_tokens!(0: A, 1: B, 2: C);
#[cfg(feature = "alloc")]
impl_assume_tokens!(0: A, 1: B, 2: C, 3: D);
#[cfg(feature = "alloc")]
impl_assume_tokens!(0: A, 1: B, 2: C, 3: D, 4: E);
#[cfg(feature = "alloc")]
impl_assume_tokens!(0: A, 1: B, 2: C, 3: D, 4: E, 5: F);
#[cfg(feature = "alloc")]
impl_assume_tokens!(0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G);
#[cfg(feature = "alloc")]
impl_assume_tokens!(0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H);
#[cfg(feature = "alloc")]
impl_assume_tokens!(0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I);
#[cfg(feature = "alloc")]
impl_assume_tokens!(
    0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J
);
#[cfg(feature = "alloc")]
impl_assume_tokens!(
    0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K
);
#[cfg(feature = "alloc")]
impl_assume_tokens!(
    0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K, 11: L
);
#[cfg(feature = "alloc")]
impl_assume_tokens!(
    0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K,
    11: L, 12: M
);
#[cfg(feature = "alloc")]
impl_assume_tokens!(
    0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K,
    11: L, 12: M, 13: N
);
#[cfg(feature = "alloc")]
impl_assume_tokens!(
    0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K,
    11: L, 12: M, 13: N, 14: O
);
#[cfg(feature = "alloc")]
impl_assume_tokens!(
    0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K,
    11: L, 12: M, 13: N, 14: O, 15: P
);
#[cfg(feature = "alloc")]
impl_assume_tokens!(
    0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K,
    11: L, 12: M, 13: N, 14: O, 15: P, 16: Q
);
#[cfg(feature = "alloc")]
impl_assume_tokens!(
    0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K,
    11: L, 12: M, 13: N, 14: O, 15: P, 16: Q, 17: R
);
#[cfg(feature = "alloc")]
impl_assume_tokens!(
    0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K,
    11: L, 12: M, 13: N, 14: O, 15: P, 16: Q, 17: R, 18: S
);
#[cfg(feature = "alloc")]
impl_assume_tokens!(
    0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K,
    11: L, 12: M, 13: N, 14: O, 15: P, 16: Q, 17: R, 18: S, 19: T
);

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

/// A `Signed<Opaque>` is a token whose payload type is erased, so as a
/// signed-token preimage `Opaque` is `None`. Its `INFO` type is
/// unreachable in practice — `Opaque` has no XDR impls, so it cannot
/// appear in a method signature outside `Signed<Opaque>`, whose spec node
/// uses only `SIGNED_PREIMAGE`.
impl groundhog_contract_spec::ContractTypeRefSpec for Opaque {
    const INFO: groundhog_contract_spec::ContractTypeInfo<'static> =
        groundhog_contract_spec::ContractTypeInfo {
            ty: groundhog_contract_spec::view::ContractTypeView::Void,
        };

    const SIGNED_PREIMAGE: Option<
        &'static groundhog_contract_spec::view::ContractTypeView<'static>,
    > = None;
}

/// A `Signed<T>` consumes no calldata bytes (the handle arrives out of
/// band); its spec node is `SignedToken` carrying `T`'s type as the token
/// preimage (`None` for `Signed<Opaque>`), so spec-driven encoders neither
/// reserve calldata bytes for it nor lose the preimage shape.
impl<T> groundhog_contract_spec::ContractTypeRefSpec for Signed<T>
where
    T: groundhog_contract_spec::ContractTypeRefSpec,
{
    const INFO: groundhog_contract_spec::ContractTypeInfo<'static> =
        groundhog_contract_spec::ContractTypeInfo {
            ty: groundhog_contract_spec::view::ContractTypeView::SignedToken(
                T::SIGNED_PREIMAGE,
            ),
        };

    #[cfg_attr(target_arch = "riscv32", inline(never))]
    #[cfg_attr(
        target_arch = "riscv32",
        unsafe(link_section = "groundhogspecanchortextv0")
    )]
    fn __groundhog_spec_anchor() -> usize {
        groundhog_contract_spec::mix_anchor_fn(
            0,
            <T as groundhog_contract_spec::ContractTypeRefSpec>::__GROUNDHOG_SPEC_ANCHOR_ROOT,
        )
    }
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
        let node = crate::mintable::build_auth_token_node::<T>(preimage);
        children.push(node).expect("too many auth token children");
        // Placeholder handle; filled by ReceiveAuthHandles after minting.
        Signed::uninitialized()
    }
}

/// Mint an auth token whose payload is the preimage of `T`, returning a
/// [`Signed<T>`] handle to it.
///
/// The token is valid until the block height `expiry` and may only be
/// consumed by the contract named in `preimage.consumer`. Nested `Signed`
/// fields inside the preimage are minted as child tokens of this one.
///
/// # Limits
///
/// The protocol caps a token node at
/// `groundhog_xdr::auth_token::kAuthTokenMaxChildNodes` (**4**) direct
/// children, and this function does not check the cap gracefully: minting a
/// preimage with more `Signed` fields at one level **panics**. The count is
/// per node and per level, and containers do not open a new level — five
/// elements in a `Vec<Signed<u64>>` field are five children of the same
/// node, not one.
///
/// A panic in a deployed contract is a fault with no message, code or
/// location (see [#358]), so this shows up on chain as an undifferentiated
/// trap. Keep the shape of a preimage within the cap by construction, or
/// mint a nested `Signed` to add a level.
///
/// The payload also has a size limit —
/// `groundhog_xdr::auth_token::kAuthTokenMaxLen` (256 bytes) of encoded
/// `T`. That one is reported, not panicked: it throws
/// `AuthTokenError::MintEncodeFailure`.
///
/// Aborts the transaction if the `MintAuthToken` syscall fails.
///
/// [#358]: https://github.com/groundhogchain/sdk_rs/issues/358
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
    let root_node = crate::mintable::build_auth_token_node::<T>(preimage);
    let buf = crate::mintable::encode_auth_token_node(&root_node);

    mint_encoded_node(expiry, buf.as_ref())
}

/// Mint an auth token from an already-encoded [`AuthTokenNode`], returning a
/// type-erased [`Signed<Opaque>`] handle.
///
/// This is the untyped counterpart of [`mint_auth_token`], for callers that
/// build or forward token nodes without knowing the payload type (see
/// [`crate::mintable::encode_auth_token_node`]). The token is valid until the
/// block height `expiry`.
///
/// Aborts the transaction if the `MintAuthToken` syscall fails.
///
/// [`AuthTokenNode`]: crate::types::AuthTokenNode
#[cfg(feature = "alloc")]
pub fn mint_opaque_auth_token(
    expiry: groundhog_xdr::types::BlockHeight,
    encoded_node: &[u8],
) -> Signed<Opaque> {
    mint_encoded_node(expiry, encoded_node)
}

/// Shared `MintAuthToken` syscall path for the typed and opaque mint fns.
#[cfg(feature = "alloc")]
fn mint_encoded_node<T>(
    expiry: groundhog_xdr::types::BlockHeight,
    encoded_node: &[u8],
) -> Signed<T> {
    use crate::sys::SyscallNo;

    let mut token_id: core::mem::MaybeUninit<u32> =
        core::mem::MaybeUninit::uninit();

    crate::sys::uninit_MintAuthToken(&expiry, encoded_node, &mut token_id)
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
    /// Consume the auth token, returning its decoded payload and the
    /// address of the contract that **minted** it.
    ///
    /// # Security
    ///
    /// Any contract can mint a token whose payload decodes as `T`.
    /// Consuming proves only that the token was (a) minted by the returned
    /// address and (b) destined for this contract. **You must check the
    /// returned minter address against an address you trust** before acting
    /// on the payload — skipping that check is an authorization bypass.
    ///
    /// # Aborts
    ///
    /// Aborts the transaction (rather than returning an error) if the
    /// `ConsumeAuthToken` syscall fails — e.g. this contract is not the
    /// token's consumer, the token is expired, or the handle was already
    /// consumed — and throws [`AuthTokenError`] if
    /// the payload fails to decode as `T` or the token's child-token count
    /// does not match the `Signed` fields of `T`. Child handles are
    /// assigned to `T`'s `Signed` fields in declaration order.
    pub fn consume(self) -> (T, ContractAddress) {
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

        let mut minter_address: MaybeUninit<ContractAddress> =
            MaybeUninit::uninit();

        crate::sys::uninit_ConsumeAuthToken(
            self.handle,
            payload_buf.spare_capacity_mut(),
            &mut minter_address,
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
        let minter_address = unsafe { minter_address.assume_init() };

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

        if iter.next().is_some() {
            crate::exn::library_throw(
                crate::AuthTokenError::ReceiveAuthHandlesFailure(
                    FaultString::from_display(
                        "unexpected auth handle remaining",
                    ),
                ),
            )
        }

        (value, minter_address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "alloc")]
    use alloc::vec;
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

    #[test]
    fn signed_spec_is_signed_token_of_payload_type() {
        use groundhog_contract_spec::type_ref;
        use groundhog_contract_spec::view::ContractTypeView;

        assert_eq!(
            type_ref::<Signed<u64>>(),
            ContractTypeView::SignedToken(Some(&ContractTypeView::U64))
        );
        // Erased payload specs as SignedToken(None).
        assert_eq!(
            type_ref::<Signed<Opaque>>(),
            ContractTypeView::SignedToken(None)
        );
        // Nested tokens spec recursively.
        assert_eq!(
            type_ref::<Signed<Signed<u32>>>(),
            ContractTypeView::SignedToken(Some(
                &ContractTypeView::SignedToken(Some(&ContractTypeView::U32))
            ))
        );
    }

    #[test]
    fn assume_preserves_handle() {
        let opaque: Signed<Opaque> = Signed::from_handle(42);
        let typed: Signed<u64> = opaque.assume();
        assert_eq!(typed.handle(), 42);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn try_assume_tokens_unit() {
        use crate::signer::AssumeTokens;
        let tokens = vec![];
        let () = AssumeTokens::try_assume_tokens(tokens).unwrap();
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn try_assume_tokens_single() {
        use crate::signer::AssumeTokens;
        let tokens = vec![Signed::<Opaque>::from_handle(10)];
        let (a,): (Signed<u64>,) =
            AssumeTokens::try_assume_tokens(tokens).unwrap();
        assert_eq!(a.handle(), 10);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn try_assume_tokens_triple() {
        use crate::signer::AssumeTokens;
        let tokens = vec![
            Signed::<Opaque>::from_handle(10),
            Signed::<Opaque>::from_handle(20),
            Signed::<Opaque>::from_handle(30),
        ];
        let (a, b, c): (Signed<u64>, Signed<u32>, Signed<bool>) =
            AssumeTokens::try_assume_tokens(tokens).unwrap();
        assert_eq!(a.handle(), 10);
        assert_eq!(b.handle(), 20);
        assert_eq!(c.handle(), 30);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn try_assume_tokens_pair() {
        use crate::signer::AssumeTokens;
        let tokens = vec![
            Signed::<Opaque>::from_handle(10),
            Signed::<Opaque>::from_handle(20),
        ];
        let (a, b): (Signed<u64>, Signed<u32>) =
            AssumeTokens::try_assume_tokens(tokens).unwrap();
        assert_eq!(a.handle(), 10);
        assert_eq!(b.handle(), 20);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn try_assume_tokens_too_many_is_decode_error() {
        use crate::exn::CallingError;
        use crate::signer::AssumeTokens;
        let tokens = vec![
            Signed::<Opaque>::from_handle(1),
            Signed::<Opaque>::from_handle(2),
            Signed::<Opaque>::from_handle(3),
        ];
        let r: Result<(Signed<u64>, Signed<u32>), CallingError> =
            AssumeTokens::try_assume_tokens(tokens);
        assert!(
            matches!(r, Err(CallingError::DecodingSuccess(_))),
            "expected DecodingSuccess, got {r:?}"
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn try_assume_tokens_too_few_is_decode_error() {
        use crate::exn::CallingError;
        use crate::signer::AssumeTokens;
        let tokens = vec![Signed::<Opaque>::from_handle(1)];
        let r: Result<(Signed<u64>, Signed<u32>), CallingError> =
            AssumeTokens::try_assume_tokens(tokens);
        assert!(
            matches!(r, Err(CallingError::DecodingSuccess(_))),
            "expected DecodingSuccess, got {r:?}"
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn try_assume_tokens_unit_rejects_nonempty() {
        use crate::exn::CallingError;
        use crate::signer::AssumeTokens;
        let tokens = vec![Signed::<Opaque>::from_handle(1)];
        let r: Result<(), CallingError> =
            AssumeTokens::try_assume_tokens(tokens);
        assert!(
            matches!(r, Err(CallingError::DecodingSuccess(_))),
            "expected DecodingSuccess, got {r:?}"
        );
    }
}
