const AUTH_HANDLES_MAX: usize = 32;

pub type AuthHandleBuf = heapless::Vec<u32, AUTH_HANDLES_MAX>;

pub trait CollectAuthHandles {
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf);
}

macro_rules! impl_collect_noop {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl CollectAuthHandles for $ty {
                #[inline(always)]
                fn collect_auth_handles(&self, _out: &mut AuthHandleBuf) {}
            }
        )+
    };
}

impl_collect_noop!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, bool, (),);

impl CollectAuthHandles for str {
    #[inline(always)]
    fn collect_auth_handles(&self, _out: &mut AuthHandleBuf) {}
}

impl<const N: usize> CollectAuthHandles for [u8; N] {
    #[inline(always)]
    fn collect_auth_handles(&self, _out: &mut AuthHandleBuf) {}
}

impl<T: CollectAuthHandles + ?Sized> CollectAuthHandles for &T {
    #[inline(always)]
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf) {
        (**self).collect_auth_handles(out);
    }
}

impl<T: CollectAuthHandles> CollectAuthHandles for Option<T> {
    #[inline(always)]
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf) {
        if let Some(inner) = self {
            inner.collect_auth_handles(out);
        }
    }
}

impl<T: CollectAuthHandles> CollectAuthHandles for [T] {
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf) {
        for item in self {
            item.collect_auth_handles(out);
        }
    }
}

#[cfg(feature = "alloc")]
impl<T: CollectAuthHandles + 'static, const MAX: u32> CollectAuthHandles
    for xdrs::VecM<T, MAX>
{
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf) {
        self.0.as_slice().collect_auth_handles(out);
    }
}

impl<const MAX: u32> CollectAuthHandles for xdrs::BytesM<MAX> {
    #[inline(always)]
    fn collect_auth_handles(&self, _out: &mut AuthHandleBuf) {}
}

impl CollectAuthHandles for crate::types::ContractAddress {
    #[inline(always)]
    fn collect_auth_handles(&self, _out: &mut AuthHandleBuf) {}
}

impl CollectAuthHandles for crate::types::SHA256Hash {
    #[inline(always)]
    fn collect_auth_handles(&self, _out: &mut AuthHandleBuf) {}
}

impl CollectAuthHandles for crate::types::Ed25519PublicKey {
    #[inline(always)]
    fn collect_auth_handles(&self, _out: &mut AuthHandleBuf) {}
}

impl CollectAuthHandles for crate::types::Ed25519Signature {
    #[inline(always)]
    fn collect_auth_handles(&self, _out: &mut AuthHandleBuf) {}
}

#[cfg(feature = "alloc")]
impl<T: CollectAuthHandles + ?Sized> CollectAuthHandles
    for alloc::boxed::Box<T>
{
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf) {
        (**self).collect_auth_handles(out);
    }
}

#[cfg(feature = "alloc")]
impl<T: CollectAuthHandles> CollectAuthHandles for alloc::vec::Vec<T> {
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf) {
        self.as_slice().collect_auth_handles(out);
    }
}

macro_rules! impl_collect_tuple {
    ($(($($name:ident),+)),+ $(,)?) => {
        $(
            impl<$($name: CollectAuthHandles),+> CollectAuthHandles for ($($name,)+) {
                #[inline(always)]
                #[allow(non_snake_case)]
                fn collect_auth_handles(&self, out: &mut AuthHandleBuf) {
                    let ($($name,)+) = self;
                    $($name.collect_auth_handles(out);)+
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
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signer::Signer;

    #[test]
    fn primitives_collect_nothing() {
        let mut buf = AuthHandleBuf::new();
        42u64.collect_auth_handles(&mut buf);
        true.collect_auth_handles(&mut buf);
        (-1i32).collect_auth_handles(&mut buf);
        assert!(buf.is_empty());
    }

    #[test]
    fn signer_collects_handle() {
        let mut buf = AuthHandleBuf::new();
        let s: Signer<u32> = Signer::from_handle(99);
        s.collect_auth_handles(&mut buf);
        assert_eq!(buf.as_slice(), &[99]);
    }

    #[test]
    fn option_some_collects() {
        let mut buf = AuthHandleBuf::new();
        let s: Option<Signer<u32>> = Some(Signer::from_handle(5));
        s.collect_auth_handles(&mut buf);
        assert_eq!(buf.as_slice(), &[5]);
    }

    #[test]
    fn option_none_collects_nothing() {
        let mut buf = AuthHandleBuf::new();
        let s: Option<Signer<u32>> = None;
        s.collect_auth_handles(&mut buf);
        assert!(buf.is_empty());
    }

    #[test]
    fn slice_of_signers() {
        let mut buf = AuthHandleBuf::new();
        let signers: &[Signer<u32>] = &[
            Signer::from_handle(1),
            Signer::from_handle(2),
            Signer::from_handle(3),
        ];
        signers.collect_auth_handles(&mut buf);
        assert_eq!(buf.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn mixed_tuple_collects_only_signers() {
        let mut buf = AuthHandleBuf::new();
        let args: (u64, Signer<u32>, i32, Signer<bool>) =
            (100, Signer::from_handle(10), -1, Signer::from_handle(20));
        args.collect_auth_handles(&mut buf);
        assert_eq!(buf.as_slice(), &[10, 20]);
    }

    #[test]
    fn single_element_tuple() {
        let mut buf = AuthHandleBuf::new();
        let args: (Signer<u32>,) = (Signer::from_handle(42),);
        args.collect_auth_handles(&mut buf);
        assert_eq!(buf.as_slice(), &[42]);
    }

    #[test]
    fn tuple_with_no_signers() {
        let mut buf = AuthHandleBuf::new();
        let args: (u64, i32, bool) = (1, 2, true);
        args.collect_auth_handles(&mut buf);
        assert!(buf.is_empty());
    }
}
