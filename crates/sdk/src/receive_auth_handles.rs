use crate::{AuthTokenError, FaultString};

pub fn receive_auth_handles_from_slice<T: ReceiveAuthHandles>(
    value: &mut T,
    handles: &[u32],
) -> Result<(), AuthTokenError> {
    let mut iter = handles.iter().copied();
    value.receive_auth_handles(&mut iter)?;

    if iter.next().is_some() {
        Err(AuthTokenError::ReceiveAuthHandlesFailure(
            FaultString::from_display("unexpected auth handle remaining"),
        ))
    } else {
        Ok(())
    }
}

/// Re-attach auth-token handles to every [`Signed`](crate::signer::Signed)
/// value nested in `self`, drawing them from `handles` in **declaration
/// order**. `Signed` decodes from zero XDR bytes with a sentinel handle;
/// this trait fills in the real handles delivered out-of-band.
///
/// This is the read half of an order-matching pair with
/// [`CollectAuthHandles`](crate::collect_auth_handles::CollectAuthHandles):
/// implementations of the two traits for a type MUST visit `Signed` fields
/// in the identical order — a mismatch silently assigns auth tokens to the
/// wrong fields. Derived implementations (via `#[contract_type]`) visit
/// struct fields in declaration order.
///
/// Errors if `handles` runs out before every `Signed` field is filled.
/// Callers that own the full handle list (the entrypoint path via
/// [`receive_auth_handles_from_slice`], and
/// [`Signed::consume`](crate::signer::Signed::consume)) additionally reject
/// leftover handles after the walk.
pub trait ReceiveAuthHandles {
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError>;
}

macro_rules! impl_receive_noop {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ReceiveAuthHandles for $ty {
                #[inline(always)]
                fn receive_auth_handles(&mut self, _handles: &mut impl Iterator<Item = u32>) -> Result<(), AuthTokenError> {
                    Ok(())
                }
            }
        )+
    };
}

impl_receive_noop!(
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
    sdk::types::ContractAddress,
    sdk::types::BlockHeight,
    sdk::types::TimePoint,
    sdk::types::SHA256Hash,
    sdk::types::Ed25519PublicKey,
    sdk::types::Ed25519Signature,
    groundhog_u256::U256,
    groundhog_u256::I256,
    sdk::types::ContractEntrypointID,
);

impl<T: ReceiveAuthHandles, const N: usize> ReceiveAuthHandles for [T; N] {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError> {
        for item in self {
            item.receive_auth_handles(handles)?;
        }
        Ok(())
    }
}

impl<T: ReceiveAuthHandles + ?Sized> ReceiveAuthHandles for &mut T {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError> {
        (**self).receive_auth_handles(handles)
    }
}

impl<T: ReceiveAuthHandles> ReceiveAuthHandles for Option<T> {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError> {
        if let Some(inner) = self {
            return inner.receive_auth_handles(handles);
        }
        Ok(())
    }
}

impl<T: ReceiveAuthHandles> ReceiveAuthHandles for [T] {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError> {
        for item in self {
            item.receive_auth_handles(handles)?;
        }
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl<T: ReceiveAuthHandles + 'static, const MAX: u32> ReceiveAuthHandles
    for xdrs::VecM<T, MAX>
{
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError> {
        for item in self.0.iter_mut() {
            item.receive_auth_handles(handles)?;
        }
        Ok(())
    }
}

impl<const MAX: u32> ReceiveAuthHandles for xdrs::BytesM<MAX> {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        _handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError> {
        Ok(())
    }
}

impl<const MAX: usize, T: ReceiveAuthHandles> ReceiveAuthHandles
    for heapless::Vec<T, MAX>
{
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError> {
        for item in self {
            item.receive_auth_handles(handles)?;
        }
        Ok(())
    }
}

impl<const MAX: usize> ReceiveAuthHandles for heapless::String<MAX> {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        _handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError> {
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl<T: ReceiveAuthHandles + ?Sized> ReceiveAuthHandles
    for alloc::boxed::Box<T>
{
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError> {
        (**self).receive_auth_handles(handles)
    }
}

impl<T: ReceiveAuthHandles + ?Sized> ReceiveAuthHandles for &T {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        _handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError> {
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl<T: ReceiveAuthHandles> ReceiveAuthHandles for alloc::vec::Vec<T> {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError> {
        self.as_mut_slice().receive_auth_handles(handles)
    }
}

macro_rules! impl_receive_tuple {
    ($(($($name:ident),+)),+ $(,)?) => {
        $(
            impl<$($name: ReceiveAuthHandles),+> ReceiveAuthHandles for ($($name,)+) {
                #[inline(always)]
                #[allow(non_snake_case)]
                fn receive_auth_handles(&mut self, handles: &mut impl Iterator<Item = u32>) -> Result<(), AuthTokenError> {
                    let ($($name,)+) = self;
                    $($name.receive_auth_handles(handles)?;)+
                    Ok(())
                }
            }
        )+
    };
}

impl_receive_tuple!(
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
    fn primitives_receive_nothing() {
        let mut x = 42u64;
        let mut handles = core::iter::empty();
        x.receive_auth_handles(&mut handles).unwrap();
    }

    #[test]
    fn signer_receives_handle() {
        let mut s: Signed<u32> = Signed::from_handle(u32::MAX);
        let mut iter = [99u32].into_iter();
        s.receive_auth_handles(&mut iter).unwrap();
        assert_eq!(s.handle(), 99);
    }

    #[test]
    fn option_some_receives() {
        let mut s: Option<Signed<u32>> = Some(Signed::from_handle(u32::MAX));
        let mut iter = [5u32].into_iter();
        s.receive_auth_handles(&mut iter).unwrap();
        assert_eq!(s.unwrap().handle(), 5);
    }

    #[test]
    fn option_none_skips() {
        let mut s: Option<Signed<u32>> = None;
        let mut iter = [5u32].into_iter();
        s.receive_auth_handles(&mut iter).unwrap();
        // handle 5 was NOT consumed
        assert_eq!(iter.next(), Some(5));
    }

    #[test]
    fn mixed_tuple_receives_in_order() {
        let mut args: (u64, Signed<u32>, i32, Signed<bool>) = (
            100,
            Signed::from_handle(u32::MAX),
            -1,
            Signed::from_handle(u32::MAX),
        );
        let mut iter = [10u32, 20u32].into_iter();
        args.receive_auth_handles(&mut iter).unwrap();
        assert_eq!(args.1.handle(), 10);
        assert_eq!(args.3.handle(), 20);
    }

    #[test]
    fn array_receives_handles_in_order() {
        let mut arr: [Signed<u32>; 2] =
            [Signed::from_handle(u32::MAX), Signed::from_handle(u32::MAX)];
        let mut iter = [7u32, 8u32].into_iter();
        arr.receive_auth_handles(&mut iter).unwrap();
        assert_eq!(arr[0].handle(), 7);
        assert_eq!(arr[1].handle(), 8);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn struct_field_after_array_gets_correct_handle() {
        // Regression: the old [T; N] impl was a no-op, so a Signed field
        // declared after an array field received the array's handles.
        let mut args: ([Signed<u32>; 2], Signed<bool>) = (
            [Signed::from_handle(u32::MAX), Signed::from_handle(u32::MAX)],
            Signed::from_handle(u32::MAX),
        );
        let mut iter = [1u32, 2u32, 3u32].into_iter();
        args.receive_auth_handles(&mut iter).unwrap();
        assert_eq!(args.0[0].handle(), 1);
        assert_eq!(args.0[1].handle(), 2);
        assert_eq!(args.1.handle(), 3);
    }

    #[test]
    fn receive_insufficient_handles() {
        let mut args: (u64, Signed<u32>, i32, Signed<bool>) = (
            100,
            Signed::from_handle(u32::MAX),
            -1,
            Signed::from_handle(u32::MAX),
        );
        let mut iter = [10u32].into_iter();
        args.receive_auth_handles(&mut iter).unwrap_err();
    }
}
