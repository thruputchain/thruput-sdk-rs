use crate::signer_mapper::MintableType;
use crate::xdr::*;

#[cfg(feature = "alloc")]
use crate::signer_mapper::SignerMapper;
#[cfg(feature = "alloc")]
use crate::types::{AuthTokenNode, AuthTokenPayload, ContractAddress};

#[cfg(feature = "alloc")]
pub struct ForMinting;
#[cfg(feature = "alloc")]
impl SignerMapper for ForMinting {
    type Map<T: MintableType> = <T as MintableType>::Unsigned;
}

pub struct UnsignedSigner<T> {
    pub consumer: crate::types::ContractAddress,
    pub data: T,
}

impl<T: WriteXDR> WriteXDR for UnsignedSigner<T> {
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        self.consumer.write_xdr(w)?;
        self.data.write_xdr(w)
    }
}

impl<'r, T: ReadXDR<'r>> ReadXDR<'r> for UnsignedSigner<T> {
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

pub trait Mintable: MintableType {
    fn mint_into_parent(
        unsigned: Self::Unsigned,
        children: &mut ChildNodeBuf,
    ) -> Self;
}

#[cfg(feature = "alloc")]
pub fn build_auth_token_node<T: Mintable>(
    unsigned: T::Unsigned,
) -> AuthTokenNode {
    let mut children = ChildNodeBuf::new();
    let payload_value = T::mint_into_parent(unsigned, &mut children);
    AuthTokenNode {
        consumer_address: ContractAddress::default(),
        payload: encode_payload(&payload_value),
        child_nodes: children_to_vecm(children),
    }
}

#[cfg(feature = "alloc")]
pub fn encode_payload<T: WriteXDR>(value: &T) -> AuthTokenPayload {
    let buf = crate::call::internal::contract_call_encode(value)
        .unwrap_or_else(|e| {
            crate::exn::library_throw(
                crate::AuthTokenError::MintEncodeFailure(
                    crate::exn::FaultString::from_display(e),
                ),
            )
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

// ── MintableType impls (no alloc needed) ──

macro_rules! impl_mintable_type_leaf {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl MintableType for $ty {
                type Unsigned = $ty;
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
    type Unsigned = BytesM<MAX>;
}

impl<const N: usize> MintableType for [u8; N] {
    type Unsigned = [u8; N];
}

impl<T: MintableType> MintableType for Option<T> {
    type Unsigned = Option<T::Unsigned>;
}

#[cfg(feature = "alloc")]
impl<T: MintableType + 'static, const MAX: u32> MintableType for VecM<T, MAX> {
    type Unsigned = VecM<T::Unsigned, MAX>;
}

#[cfg(feature = "alloc")]
impl<T: MintableType> MintableType for alloc::vec::Vec<T> {
    type Unsigned = alloc::vec::Vec<T::Unsigned>;
}

#[cfg(feature = "alloc")]
impl<T: MintableType + ?Sized> MintableType for alloc::boxed::Box<T> {
    type Unsigned = alloc::boxed::Box<T::Unsigned>;
}

// ── Mintable impls ──

macro_rules! impl_mintable_leaf {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl Mintable for $ty {
                fn mint_into_parent(unsigned: $ty, _children: &mut ChildNodeBuf) -> Self {
                    unsigned
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
        unsigned: BytesM<MAX>,
        _children: &mut ChildNodeBuf,
    ) -> Self {
        unsigned
    }
}

impl<const N: usize> Mintable for [u8; N] {
    fn mint_into_parent(
        unsigned: [u8; N],
        _children: &mut ChildNodeBuf,
    ) -> Self {
        unsigned
    }
}

impl<T: Mintable> Mintable for Option<T> {
    fn mint_into_parent(
        unsigned: Option<T::Unsigned>,
        children: &mut ChildNodeBuf,
    ) -> Self {
        unsigned.map(|inner| T::mint_into_parent(inner, children))
    }
}

#[cfg(feature = "alloc")]
impl<T: Mintable + 'static, const MAX: u32> Mintable for VecM<T, MAX>
where
    T::Unsigned: 'static,
{
    fn mint_into_parent(
        unsigned: VecM<T::Unsigned, MAX>,
        children: &mut ChildNodeBuf,
    ) -> Self {
        let v: alloc::vec::Vec<T> = unsigned
            .0
            .into_iter()
            .map(|item| T::mint_into_parent(item, children))
            .collect();
        VecM(v)
    }
}

#[cfg(feature = "alloc")]
impl<T: Mintable> Mintable for alloc::vec::Vec<T> {
    fn mint_into_parent(
        unsigned: alloc::vec::Vec<T::Unsigned>,
        children: &mut ChildNodeBuf,
    ) -> Self {
        unsigned
            .into_iter()
            .map(|item| T::mint_into_parent(item, children))
            .collect()
    }
}

#[cfg(feature = "alloc")]
impl<T: Mintable + ?Sized> Mintable for alloc::boxed::Box<T>
where
    T::Unsigned: Sized,
{
    fn mint_into_parent(
        unsigned: alloc::boxed::Box<T::Unsigned>,
        children: &mut ChildNodeBuf,
    ) -> Self {
        alloc::boxed::Box::new(T::mint_into_parent(*unsigned, children))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signer::Signer;

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
    fn signer_mintable_sets_consumer() {
        // Simulate what mint_auth_token does: build inner node, set consumer
        let consumer = crate::types::ContractAddress::default();
        let mut node = build_auth_token_node::<u64>(42u64);
        node.consumer_address = consumer;
        assert_eq!(node.consumer_address, consumer);
        assert!(node.child_nodes.is_empty());
    }

    #[test]
    fn signer_mint_into_parent_pushes_child() {
        let mut children = ChildNodeBuf::new();
        let consumer = crate::types::ContractAddress::default();
        let val = <Signer<u64>>::mint_into_parent(
            UnsignedSigner {
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
        let val = <Option<Signer<u64>>>::mint_into_parent(None, &mut children);
        assert!(val.is_none());
        assert!(children.is_empty());
    }

    #[test]
    fn option_some_signer_pushes_child() {
        let mut children = ChildNodeBuf::new();
        let consumer = crate::types::ContractAddress::default();
        let val = <Option<Signer<u64>>>::mint_into_parent(
            Some(UnsignedSigner {
                consumer,
                data: 42u64,
            }),
            &mut children,
        );
        assert!(val.is_some());
        assert_eq!(children.len(), 1);
    }
}
