pub fn receive_auth_handles_from_slice<T: ReceiveAuthHandles>(
    value: &mut T,
    handles: &[u32],
) {
    let mut iter = handles.iter().copied();
    value.receive_auth_handles(&mut iter);
}

pub trait ReceiveAuthHandles {
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    );
}

macro_rules! impl_receive_noop {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ReceiveAuthHandles for $ty {
                #[inline(always)]
                fn receive_auth_handles(&mut self, _handles: &mut impl Iterator<Item = u32>) {}
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
    sdk::types::SHA256Hash,
    sdk::types::Ed25519PublicKey,
    sdk::types::Ed25519Signature,
    sdk::types::ContractEntrypointID,
);

impl<const N: usize> ReceiveAuthHandles for [u8; N] {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        _handles: &mut impl Iterator<Item = u32>,
    ) {
    }
}

impl<T: ReceiveAuthHandles + ?Sized> ReceiveAuthHandles for &mut T {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    ) {
        (**self).receive_auth_handles(handles);
    }
}

impl<T: ReceiveAuthHandles> ReceiveAuthHandles for Option<T> {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    ) {
        if let Some(inner) = self {
            inner.receive_auth_handles(handles);
        }
    }
}

impl<T: ReceiveAuthHandles> ReceiveAuthHandles for [T] {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    ) {
        for item in self {
            item.receive_auth_handles(handles);
        }
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
    ) {
        for item in self.0.iter_mut() {
            item.receive_auth_handles(handles);
        }
    }
}

impl<const MAX: u32> ReceiveAuthHandles for xdrs::BytesM<MAX> {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        _handles: &mut impl Iterator<Item = u32>,
    ) {
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
    ) {
        (**self).receive_auth_handles(handles);
    }
}

impl<T: ReceiveAuthHandles + ?Sized> ReceiveAuthHandles for &T {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        _handles: &mut impl Iterator<Item = u32>,
    ) {
    }
}

#[cfg(feature = "alloc")]
impl<T: ReceiveAuthHandles> ReceiveAuthHandles for alloc::vec::Vec<T> {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        handles: &mut impl Iterator<Item = u32>,
    ) {
        self.as_mut_slice().receive_auth_handles(handles);
    }
}

macro_rules! impl_receive_tuple {
    ($(($($name:ident),+)),+ $(,)?) => {
        $(
            impl<$($name: ReceiveAuthHandles),+> ReceiveAuthHandles for ($($name,)+) {
                #[inline(always)]
                #[allow(non_snake_case)]
                fn receive_auth_handles(&mut self, handles: &mut impl Iterator<Item = u32>) {
                    let ($($name,)+) = self;
                    $($name.receive_auth_handles(handles);)+
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
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signer::Signed;

    #[test]
    fn primitives_receive_nothing() {
        let mut x = 42u64;
        let mut handles = core::iter::empty();
        x.receive_auth_handles(&mut handles);
    }

    #[test]
    fn signer_receives_handle() {
        let mut s: Signed<u32> = Signed::from_handle(u32::MAX);
        let mut iter = [99u32].into_iter();
        s.receive_auth_handles(&mut iter);
        assert_eq!(s.handle(), 99);
    }

    #[test]
    fn option_some_receives() {
        let mut s: Option<Signed<u32>> = Some(Signed::from_handle(u32::MAX));
        let mut iter = [5u32].into_iter();
        s.receive_auth_handles(&mut iter);
        assert_eq!(s.unwrap().handle(), 5);
    }

    #[test]
    fn option_none_skips() {
        let mut s: Option<Signed<u32>> = None;
        let mut iter = [5u32].into_iter();
        s.receive_auth_handles(&mut iter);
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
        args.receive_auth_handles(&mut iter);
        assert_eq!(args.1.handle(), 10);
        assert_eq!(args.3.handle(), 20);
    }
}
