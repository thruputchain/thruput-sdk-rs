use crate::FaultString;

#[cfg(not(feature = "alloc"))]
const AUTH_HANDLES_MAX: usize = 64;
#[cfg(not(feature = "alloc"))]
pub type AuthHandleBuf = heapless::Vec<u32, AUTH_HANDLES_MAX>;
#[cfg(feature = "alloc")]
pub type AuthHandleBuf = alloc::vec::Vec<u32>;

/// Extract the auth-token handles of every [`Signed`](crate::signer::Signed)
/// value nested in `self`, in **declaration order**, so they can travel
/// alongside the XDR payload of a contract call (`Signed` encodes to zero
/// bytes; the handles are passed out-of-band).
///
/// This is the write half of an order-matching pair with
/// [`ReceiveAuthHandles`](crate::receive_auth_handles::ReceiveAuthHandles):
/// the callee re-attaches handles to its decoded `Signed` fields by walking
/// the same structure in the same order. Implementations of the two traits
/// for a type MUST visit `Signed` fields in the identical order — a mismatch
/// silently assigns auth tokens to the wrong fields. Derived implementations
/// (via `#[contract_type]`) visit struct fields in declaration order.
pub trait CollectAuthHandles {
    fn collect_auth_handles(
        &self,
        out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString>;
}

macro_rules! impl_collect_noop {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl CollectAuthHandles for $ty {
                #[inline(always)]
                fn collect_auth_handles(&self, _out: &mut AuthHandleBuf) -> Result<(), FaultString> {
                    Ok(())
                }
            }
        )+
    };
}

impl_collect_noop!(
    u8,
    u16,
    u32,
    u64,
    u128,
    i8,
    i16,
    i32,
    i64,
    i128,
    bool,
    (),
    str,
    sdk::types::ContractAddress,
    sdk::types::BlockHeight,
    sdk::types::SHA256Hash,
    sdk::types::Ed25519PublicKey,
    sdk::types::Ed25519Signature,
    groundhog_u256::U256,
    groundhog_u256::I256,
    sdk::types::ContractEntrypointID,
);

impl<T: CollectAuthHandles, const N: usize> CollectAuthHandles for [T; N] {
    #[inline(always)]
    fn collect_auth_handles(
        &self,
        out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString> {
        for item in self {
            item.collect_auth_handles(out)?;
        }
        Ok(())
    }
}

impl<T: CollectAuthHandles + ?Sized> CollectAuthHandles for &T {
    #[inline(always)]
    fn collect_auth_handles(
        &self,
        out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString> {
        (**self).collect_auth_handles(out)
    }
}

impl<T: CollectAuthHandles> CollectAuthHandles for Option<T> {
    #[inline(always)]
    fn collect_auth_handles(
        &self,
        out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString> {
        if let Some(inner) = self {
            inner.collect_auth_handles(out)?;
        }
        Ok(())
    }
}

impl<T: CollectAuthHandles> CollectAuthHandles for [T] {
    #[inline(always)]
    fn collect_auth_handles(
        &self,
        out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString> {
        for item in self {
            item.collect_auth_handles(out)?;
        }
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl<T: CollectAuthHandles, const MAX: u32> CollectAuthHandles
    for xdrs::VecM<T, MAX>
{
    #[inline(always)]
    fn collect_auth_handles(
        &self,
        out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString> {
        self.0.as_slice().collect_auth_handles(out)
    }
}

impl<const MAX: u32> CollectAuthHandles for xdrs::BytesM<MAX> {
    #[inline(always)]
    fn collect_auth_handles(
        &self,
        _out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString> {
        Ok(())
    }
}

impl<const MAX: usize, T: CollectAuthHandles> CollectAuthHandles
    for heapless::Vec<T, MAX>
{
    #[inline(always)]
    fn collect_auth_handles(
        &self,
        out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString> {
        for item in self {
            item.collect_auth_handles(out)?;
        }
        Ok(())
    }
}

impl<const MAX: usize> CollectAuthHandles for heapless::String<MAX> {
    #[inline(always)]
    fn collect_auth_handles(
        &self,
        _out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString> {
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl<T: CollectAuthHandles + ?Sized> CollectAuthHandles
    for alloc::boxed::Box<T>
{
    #[inline(always)]
    fn collect_auth_handles(
        &self,
        out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString> {
        (**self).collect_auth_handles(out)
    }
}

#[cfg(feature = "alloc")]
impl<T: CollectAuthHandles> CollectAuthHandles for alloc::vec::Vec<T> {
    #[inline(always)]
    fn collect_auth_handles(
        &self,
        out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString> {
        self.as_slice().collect_auth_handles(out)
    }
}

macro_rules! impl_collect_tuple {
    ($(($($name:ident),+)),+ $(,)?) => {
        $(
            impl<$($name: CollectAuthHandles),+> CollectAuthHandles for ($($name,)+) {
                #[inline(always)]
                #[allow(non_snake_case)]
                fn collect_auth_handles(&self, out: &mut AuthHandleBuf) -> Result<(), FaultString> {
                    let ($($name,)+) = self;
                    $($name.collect_auth_handles(out)?;)+
                    Ok(())
                }
            }
        )+
    };
}

impl_collect_tuple!(
    (A),
    (A, B),
    (A, B, C),
    (A, B, C, D),
    (A, B, C, D, E),
    (A, B, C, D, E, F),
    (A, B, C, D, E, F, G),
    (A, B, C, D, E, F, G, H),
    (A, B, C, D, E, F, G, H, I),
    (A, B, C, D, E, F, G, H, I, J),
    (A, B, C, D, E, F, G, H, I, J, K),
    (A, B, C, D, E, F, G, H, I, J, K, L),
    (A, B, C, D, E, F, G, H, I, J, K, L, M),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T),
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signer::Signed;

    #[test]
    fn primitives_collect_nothing() {
        let mut buf = AuthHandleBuf::new();
        42u64.collect_auth_handles(&mut buf).unwrap();
        true.collect_auth_handles(&mut buf).unwrap();
        (-1i32).collect_auth_handles(&mut buf).unwrap();
        assert!(buf.is_empty());
    }

    #[test]
    fn signer_collects_handle() {
        let mut buf = AuthHandleBuf::new();
        let s: Signed<u32> = Signed::from_handle(99);
        s.collect_auth_handles(&mut buf).unwrap();
        assert_eq!(buf.as_slice(), &[99]);
    }

    #[test]
    fn option_some_collects() {
        let mut buf = AuthHandleBuf::new();
        let s: Option<Signed<u32>> = Some(Signed::from_handle(5));
        s.collect_auth_handles(&mut buf).unwrap();
        assert_eq!(buf.as_slice(), &[5]);
    }

    #[test]
    fn option_none_collects_nothing() {
        let mut buf = AuthHandleBuf::new();
        let s: Option<Signed<u32>> = None;
        s.collect_auth_handles(&mut buf).unwrap();
        assert!(buf.is_empty());
    }

    #[test]
    fn slice_of_signeds() {
        let mut buf = AuthHandleBuf::new();
        let signeds: &[Signed<u32>] = &[
            Signed::from_handle(1),
            Signed::from_handle(2),
            Signed::from_handle(3),
        ];
        signeds.collect_auth_handles(&mut buf).unwrap();
        assert_eq!(buf.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn mixed_tuple_collects_only_signeds() {
        let mut buf = AuthHandleBuf::new();
        let args: (u64, Signed<u32>, i32, Signed<bool>) =
            (100, Signed::from_handle(10), -1, Signed::from_handle(20));
        args.collect_auth_handles(&mut buf).unwrap();
        assert_eq!(buf.as_slice(), &[10, 20]);
    }

    #[test]
    fn single_element_tuple() {
        let mut buf = AuthHandleBuf::new();
        let args: (Signed<u32>,) = (Signed::from_handle(42),);
        args.collect_auth_handles(&mut buf).unwrap();
        assert_eq!(buf.as_slice(), &[42]);
    }

    #[test]
    fn tuple_with_no_signeds() {
        let mut buf = AuthHandleBuf::new();
        let args: (u64, i32, bool) = (1, 2, true);
        args.collect_auth_handles(&mut buf).unwrap();
        assert!(buf.is_empty());
    }
}
