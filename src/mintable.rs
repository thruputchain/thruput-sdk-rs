use crate::xdr::*;

use crate::types::AuthTokenNode;

#[cfg(feature = "alloc")]
use crate::types::{AuthTokenPayload, ContractAddress};

/// Marker trait for types that have a pre-signing (preimage) representation.
///
/// `Preimage` is intentionally unbound — the `WriteXDR + ReadXDR` requirement
/// is deferred to the `Mintable` trait so that recursive contracttypes can
/// implement `MintableType` without triggering trait-solver overflow.
pub trait MintableType: Sized + WriteXDR + for<'r> ReadXDR<'r> {
    type Preimage;
}

pub struct AuthTokenPreimage<T> {
    pub consumer: crate::types::ContractAddress,
    pub data: T,
}

impl<T: WriteXDR> WriteXDR for AuthTokenPreimage<T> {
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        self.consumer.write_xdr(w)?;
        self.data.write_xdr(w)
    }
}

impl<'r, T: ReadXDR<'r>> ReadXDR<'r> for AuthTokenPreimage<T> {
    fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
        Ok(Self {
            consumer: crate::types::ContractAddress::read_xdr(r)?,
            data: T::read_xdr(r)?,
        })
    }
}

const AUTH_TOKEN_MAX_CHILDREN: usize =
    groundhog_xdr::auth_token::kAuthTokenMaxChildNodes as usize;

pub type ChildNodeBuf = heapless::Vec<AuthTokenNode, AUTH_TOKEN_MAX_CHILDREN>;

pub trait Mintable: MintableType
where
    Self::Preimage: WriteXDR + for<'r> ReadXDR<'r>,
{
    fn mint_into_parent(
        preimage: Self::Preimage,
        children: &mut ChildNodeBuf,
    ) -> Self;
}

#[cfg(feature = "alloc")]
pub fn build_auth_token_node<T: Mintable>(
    preimage: T::Preimage,
) -> AuthTokenNode
where
    T::Preimage: WriteXDR + for<'r> ReadXDR<'r>,
{
    let mut children = ChildNodeBuf::new();
    let payload_value = T::mint_into_parent(preimage, &mut children);
    AuthTokenNode {
        consumer_address: ContractAddress::default(),
        payload: encode_payload(&payload_value),
        child_nodes: children_to_vecm(children),
    }
}

#[cfg(feature = "alloc")]
pub fn encode_payload<T: WriteXDR>(value: &T) -> AuthTokenPayload {
    let buf = crate::call::contract_call_encode(value).unwrap_or_else(|e| {
        crate::exn::library_throw(crate::AuthTokenError::MintEncodeFailure(
            crate::exn::FaultString::from_display(e),
        ))
    });
    let bytes: xdrs::BytesM<{ groundhog_xdr::auth_token::kAuthTokenMaxLen }> =
        buf.as_slice().try_into().unwrap_or_else(|_| {
            crate::exn::library_throw(
                crate::AuthTokenError::MintEncodeFailure(
                    crate::exn::FaultString::from_display(
                        "auth token payload exceeds max length",
                    ),
                ),
            )
        });
    AuthTokenPayload::from(bytes)
}

#[cfg(feature = "alloc")]
fn children_to_vecm(
    children: ChildNodeBuf,
) -> xdrs::VecM<
    AuthTokenNode,
    { groundhog_xdr::auth_token::kAuthTokenMaxChildNodes },
> {
    children.as_slice().try_into().unwrap_or_else(|_| {
        crate::exn::library_throw(crate::AuthTokenError::MintEncodeFailure(
            crate::exn::FaultString::from_display(format_args!(
                "too many auth token children (max {})",
                groundhog_xdr::auth_token::kAuthTokenMaxChildNodes
            )),
        ))
    })
}

#[cfg(feature = "alloc")]
pub fn encode_auth_token_node(node: &AuthTokenNode) -> alloc::vec::Vec<u8> {
    xdrs::txs::encode(node).unwrap_or_else(|e| {
        crate::exn::library_throw(crate::AuthTokenError::MintEncodeFailure(
            crate::exn::FaultString::from_display(e),
        ))
    })
}

// ── MintableType leaf impls ──

macro_rules! impl_mintable_type_leaf {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl MintableType for $ty {
                type Preimage = $ty;
            }
        )+
    };
}

impl_mintable_type_leaf!(
    u16,
    u32,
    u64,
    u128,
    i16,
    i32,
    i64,
    i128,
    bool,
    (),
    crate::types::ContractAddress,
    crate::types::SHA256Hash,
    crate::types::Ed25519PublicKey,
    crate::types::Ed25519Signature,
);

impl<const MAX: u32> MintableType for BytesM<MAX> {
    type Preimage = BytesM<MAX>;
}

impl<const N: usize> MintableType for [u8; N] {
    type Preimage = [u8; N];
}

impl<const N: usize, T: MintableType> MintableType for [T; N] {
    type Preimage = [T::Preimage; N];
}

impl<T: MintableType> MintableType for Option<T> {
    type Preimage = Option<T::Preimage>;
}

#[cfg(feature = "alloc")]
impl<T: MintableType + 'static, const MAX: u32> MintableType for VecM<T, MAX> {
    type Preimage = VecM<T::Preimage, MAX>;
}

#[cfg(feature = "alloc")]
impl<const MAX: u32> MintableType for StringM<MAX> {
    type Preimage = StringM<MAX>;
}

#[cfg(feature = "alloc")]
impl<T: MintableType> MintableType for alloc::vec::Vec<T> {
    type Preimage = alloc::vec::Vec<T::Preimage>;
}

#[cfg(feature = "alloc")]
impl<T: MintableType> MintableType for alloc::boxed::Box<T> {
    type Preimage = alloc::boxed::Box<T::Preimage>;
}

// ── Mintable leaf impls ──

macro_rules! impl_mintable_leaf {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl Mintable for $ty {
                fn mint_into_parent(preimage: $ty, _children: &mut ChildNodeBuf) -> Self {
                    preimage
                }
            }
        )+
    };
}

impl_mintable_leaf!(
    u16,
    u32,
    u64,
    u128,
    i16,
    i32,
    i64,
    i128,
    bool,
    (),
    crate::types::ContractAddress,
    crate::types::SHA256Hash,
    crate::types::Ed25519PublicKey,
    crate::types::Ed25519Signature,
);

impl<const MAX: u32> Mintable for BytesM<MAX> {
    fn mint_into_parent(
        preimage: BytesM<MAX>,
        _children: &mut ChildNodeBuf,
    ) -> Self {
        preimage
    }
}

#[cfg(feature = "alloc")]
impl<const MAX: u32> Mintable for StringM<MAX> {
    fn mint_into_parent(
        preimage: StringM<MAX>,
        _children: &mut ChildNodeBuf,
    ) -> Self {
        preimage
    }
}

impl<const N: usize> Mintable for [u8; N] {
    fn mint_into_parent(
        preimage: [u8; N],
        _children: &mut ChildNodeBuf,
    ) -> Self {
        preimage
    }
}

impl<T: Mintable> Mintable for Option<T>
where
    T::Preimage: WriteXDR + for<'r> ReadXDR<'r>,
    Option<T::Preimage>: WriteXDR + for<'r> ReadXDR<'r>,
{
    fn mint_into_parent(
        preimage: Option<T::Preimage>,
        children: &mut ChildNodeBuf,
    ) -> Self {
        preimage.map(|inner| T::mint_into_parent(inner, children))
    }
}

#[cfg(feature = "alloc")]
impl<T: Mintable + 'static, const MAX: u32> Mintable for VecM<T, MAX>
where
    T::Preimage: WriteXDR + for<'r> ReadXDR<'r> + 'static,
    VecM<T::Preimage, MAX>: WriteXDR + for<'r> ReadXDR<'r>,
{
    fn mint_into_parent(
        preimage: VecM<T::Preimage, MAX>,
        children: &mut ChildNodeBuf,
    ) -> Self {
        let v: alloc::vec::Vec<T> = preimage
            .0
            .into_iter()
            .map(|item| T::mint_into_parent(item, children))
            .collect();
        VecM(v)
    }
}

impl<const N: usize, T: Mintable> Mintable for [T; N]
where
    T::Preimage: WriteXDR + for<'r> ReadXDR<'r>,
    [T::Preimage; N]: WriteXDR + for<'r> ReadXDR<'r>,
{
    fn mint_into_parent(
        preimage: [T::Preimage; N],
        children: &mut ChildNodeBuf,
    ) -> Self {
        preimage.map(|inner| T::mint_into_parent(inner, children))
    }
}

#[cfg(feature = "alloc")]
impl<T: Mintable> Mintable for alloc::vec::Vec<T>
where
    T::Preimage: WriteXDR + for<'r> ReadXDR<'r>,
    alloc::vec::Vec<T::Preimage>: WriteXDR + for<'r> ReadXDR<'r>,
{
    fn mint_into_parent(
        preimage: alloc::vec::Vec<T::Preimage>,
        children: &mut ChildNodeBuf,
    ) -> Self {
        preimage
            .into_iter()
            .map(|item| T::mint_into_parent(item, children))
            .collect()
    }
}

#[cfg(feature = "alloc")]
impl<T: Mintable> Mintable for alloc::boxed::Box<T>
where
    T::Preimage: Sized + WriteXDR + for<'r> ReadXDR<'r>,
    alloc::boxed::Box<T::Preimage>: WriteXDR + for<'r> ReadXDR<'r>,
{
    fn mint_into_parent(
        preimage: alloc::boxed::Box<T::Preimage>,
        children: &mut ChildNodeBuf,
    ) -> Self {
        alloc::boxed::Box::new(T::mint_into_parent(*preimage, children))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signer::Signed;

    #[test]
    fn primitive_mintable_builds_leaf_node() {
        let node = build_auth_token_node::<u64>(42u64);
        assert!(node.child_nodes.is_empty());
    }

    #[test]
    fn primitive_mint_into_parent_no_children() {
        let mut children = ChildNodeBuf::new();
        let val = u64::mint_into_parent(42u64, &mut children);
        assert_eq!(val, 42);
        assert!(children.is_empty());
    }

    #[test]
    fn signed_mintable_sets_consumer() {
        let consumer = crate::types::ContractAddress::default();
        let mut node = build_auth_token_node::<u64>(42u64);
        node.consumer_address = consumer;
        assert_eq!(node.consumer_address, consumer);
        assert!(node.child_nodes.is_empty());
    }

    #[test]
    fn signed_mint_into_parent_pushes_child() {
        let mut children = ChildNodeBuf::new();
        let consumer = crate::types::ContractAddress::default();
        let val = <Signed<u64>>::mint_into_parent(
            AuthTokenPreimage {
                consumer,
                data: 42u64,
            },
            &mut children,
        );
        assert_eq!(val.handle(), u32::MAX);
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn option_none_no_children() {
        let mut children = ChildNodeBuf::new();
        let val = <Option<Signed<u64>>>::mint_into_parent(None, &mut children);
        assert!(val.is_none());
        assert!(children.is_empty());
    }

    #[test]
    fn option_some_signed_pushes_child() {
        let mut children = ChildNodeBuf::new();
        let consumer = crate::types::ContractAddress::default();
        let val = <Option<Signed<u64>>>::mint_into_parent(
            Some(AuthTokenPreimage {
                consumer,
                data: 42u64,
            }),
            &mut children,
        );
        assert!(val.is_some());
        assert_eq!(children.len(), 1);
    }
}
