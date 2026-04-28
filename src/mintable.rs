use crate::types::{AuthTokenNode, AuthTokenPayload, ContractAddress};
use crate::xdr::*;

pub trait SignerMapper {
    type Map<T: Mintable>: WriteXDR + for<'r> ReadXDR<'r>;
}

pub struct Signed;
impl SignerMapper for Signed {
    type Map<T: Mintable> = T;
}

pub struct ForMinting;
impl SignerMapper for ForMinting {
    type Map<T: Mintable> = <T as Mintable>::Unsigned;
}

pub struct UnsignedSigner<T> {
    pub consumer: ContractAddress,
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
            consumer: ContractAddress::read_xdr(r)?,
            data: T::read_xdr(r)?,
        })
    }
}

pub trait Mintable: Sized + WriteXDR + for<'r> ReadXDR<'r> {
    type Unsigned: WriteXDR + for<'r> ReadXDR<'r>;

    fn mint_into_parent(
        unsigned: Self::Unsigned,
        children: &mut alloc::vec::Vec<AuthTokenNode>,
    ) -> Self;

    fn build_auth_token_node(unsigned: Self::Unsigned) -> AuthTokenNode {
        let mut children = alloc::vec::Vec::new();
        let payload_value = Self::mint_into_parent(unsigned, &mut children);
        AuthTokenNode {
            consumer_address: ContractAddress::default(),
            payload: encode_payload(&payload_value),
            child_nodes: auth_token_children(children),
        }
    }
}

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
            panic!("auth token payload exceeds max length")
        });
    AuthTokenPayload::from(bytes)
}

pub fn encode_auth_token_node(node: &AuthTokenNode) -> alloc::vec::Vec<u8> {
    cxs::encode(node).unwrap_or_else(|e| {
        crate::exn::library_throw(crate::AuthTokenError::MintEncodeFailure(
            crate::exn::FaultString::from_display(e),
        ))
    })
}

pub fn auth_token_children(
    nodes: alloc::vec::Vec<AuthTokenNode>,
) -> xdrs::VecM<
    AuthTokenNode,
    { groundhog_xdr::auth_token::kAuthTokenMaxChildNodes },
> {
    nodes.try_into().unwrap_or_else(|_| {
        panic!(
            "too many auth token children (max {})",
            groundhog_xdr::auth_token::kAuthTokenMaxChildNodes
        )
    })
}

macro_rules! impl_mintable_leaf {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl Mintable for $ty {
                type Unsigned = $ty;

                fn mint_into_parent(
                    unsigned: $ty,
                    _children: &mut alloc::vec::Vec<AuthTokenNode>,
                ) -> Self {
                    unsigned
                }
            }
        )+
    };
}

impl_mintable_leaf!(u16, u32, u64, u128, i16, i32, i64, i128, bool, (),);

impl Mintable for ContractAddress {
    type Unsigned = ContractAddress;
    fn mint_into_parent(
        unsigned: ContractAddress,
        _children: &mut alloc::vec::Vec<AuthTokenNode>,
    ) -> Self {
        unsigned
    }
}

impl Mintable for crate::types::SHA256Hash {
    type Unsigned = crate::types::SHA256Hash;
    fn mint_into_parent(
        unsigned: crate::types::SHA256Hash,
        _children: &mut alloc::vec::Vec<AuthTokenNode>,
    ) -> Self {
        unsigned
    }
}

impl<T: Mintable> Mintable for Option<T> {
    type Unsigned = Option<T::Unsigned>;
    fn mint_into_parent(
        unsigned: Option<T::Unsigned>,
        children: &mut alloc::vec::Vec<AuthTokenNode>,
    ) -> Self {
        unsigned.map(|inner| T::mint_into_parent(inner, children))
    }
}

impl<T: Mintable> Mintable for alloc::vec::Vec<T> {
    type Unsigned = alloc::vec::Vec<T::Unsigned>;
    fn mint_into_parent(
        unsigned: alloc::vec::Vec<T::Unsigned>,
        children: &mut alloc::vec::Vec<AuthTokenNode>,
    ) -> Self {
        unsigned
            .into_iter()
            .map(|item| T::mint_into_parent(item, children))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signer::Signer;

    #[test]
    fn primitive_mintable_builds_leaf_node() {
        let node = u64::build_auth_token_node(42u64);
        assert!(node.child_nodes.is_empty());
    }

    #[test]
    fn primitive_mint_into_parent_no_children() {
        let mut children = alloc::vec::Vec::new();
        let val = u64::mint_into_parent(42u64, &mut children);
        assert_eq!(val, 42);
        assert!(children.is_empty());
    }

    #[test]
    fn signer_mintable_sets_consumer() {
        let consumer = ContractAddress::default();
        let node = <Signer<u64>>::build_auth_token_node(UnsignedSigner {
            consumer: consumer.clone(),
            data: 42u64,
        });
        assert_eq!(node.consumer_address, consumer);
        assert!(node.child_nodes.is_empty());
    }

    #[test]
    fn signer_mint_into_parent_pushes_child() {
        let mut children = alloc::vec::Vec::new();
        let consumer = ContractAddress::default();
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
        let mut children = alloc::vec::Vec::new();
        let val = <Option<Signer<u64>>>::mint_into_parent(None, &mut children);
        assert!(val.is_none());
        assert!(children.is_empty());
    }

    #[test]
    fn option_some_signer_pushes_child() {
        let mut children = alloc::vec::Vec::new();
        let consumer = ContractAddress::default();
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
