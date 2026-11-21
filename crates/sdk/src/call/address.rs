//! [`Address<C>`]: a contract address typed by the client used to call it.

use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::ops::Deref;

use super::{ContractClient, RefArg, RefArgElement};
use crate::collect_auth_handles::{AuthHandleBuf, CollectAuthHandles};
use crate::event::{IntoTopic, TopicSrc};
use crate::mintable::{ChildNodeBuf, Mintable, MintableType};
use crate::receive_auth_handles::ReceiveAuthHandles;
use crate::types::ContractAddress;
use crate::xdr::{
    ReadResult, ReadXDR, Reader, WriteResult, WriteXDR, Writer,
    WriterUnchecked, cxs, txs,
};
use crate::{AuthTokenError, FaultString};

/// The address of a contract that is expected to implement the interface
/// client `C` calls, or each interface in a tuple of clients.
///
/// Use it wherever a [`ContractAddress`] is stored or passed and the contract
/// behind it is meant to be called through known interfaces: a storage field,
/// a `#[contract_type]` field, a map key, or a `#[contract]` method
/// parameter. The interfaces are then named once, at the declaration,
/// instead of at every call.
///
/// ```ignore
/// #[contract_type]
/// pub struct Terms {
///     pub sale_token: Address<ERC20Client>,
///     pub payment_token: Address<(ERC20Client, ERC20MetadataClient)>,
/// }
///
/// terms.sale_token.client().transfer(&bidder, amount);
///
/// ERC20Client::at(&terms.payment_token).transfer(&seller, amount);
/// let decimals = ERC20MetadataClient::at(&terms.payment_token).decimals();
/// ```
///
/// [`ContractClient::at`] names the client and works for every `Address`
/// that lists it. [`Address::client`] infers the client, so it reads best on
/// an address with one interface. Naming a client the address does not list
/// is a compile error.
///
/// An `Address<C>` encodes exactly as a [`ContractAddress`]: its wire form,
/// storage form, contract spec and event topic are the same. Changing a field
/// between the two, or changing its client list, therefore changes neither a
/// storage layout nor an ABI.
///
/// The type records an expectation; it does not check it. Decoding accepts
/// any address, and nothing here asks the contract which interfaces it
/// implements. Probe the contract where a caller-supplied address enters, if
/// the contract must not trust it.
///
/// A client's gas limit and invocation flags belong to a call, not to the
/// address, so they are not part of this type. Set them on the client.
#[repr(transparent)]
pub struct Address<C> {
    address: ContractAddress,
    // `fn() -> C` keeps `Address<C>` `Send`, `Sync` and `Copy` whatever `C`
    // is: no `C` is ever held.
    client: PhantomData<fn() -> C>,
}

impl<C> Address<C> {
    /// Type `address` as a contract called through `C`.
    pub const fn new(address: ContractAddress) -> Self {
        Self {
            address,
            client: PhantomData,
        }
    }

    /// Type a borrowed `address` as a contract called through `C`.
    pub const fn from_ref(address: &ContractAddress) -> &Self {
        // SAFETY: `Address<C>` is `repr(transparent)` over `ContractAddress`
        // (its other field is a zero-sized `PhantomData`), so the two have
        // the same layout and validity.
        unsafe { &*(address as *const ContractAddress as *const Self) }
    }

    /// The untyped address.
    pub const fn address(&self) -> &ContractAddress {
        &self.address
    }

    /// The untyped address, by value.
    pub const fn into_address(self) -> ContractAddress {
        self.address
    }

    /// The same address, retyped without a check. Prefer
    /// [`Address::narrow`] when the new client is one this address lists.
    pub const fn cast<D>(self) -> Address<D> {
        Address::new(self.address)
    }

    /// A client for the contract at this address, with the default
    /// invocation configuration.
    ///
    /// The client is inferred. An address with several interfaces needs it
    /// stated, which [`ContractClient::at`] does most directly.
    #[inline(always)]
    pub fn client<D, I>(&self) -> D
    where
        C: Provides<D, I>,
        D: ContractClient,
    {
        D::new(&self.address)
    }

    /// The same address, typed by one of the clients this address lists.
    #[inline(always)]
    pub fn narrow<D, I>(self) -> Address<D>
    where
        C: Provides<D, I>,
    {
        Address::new(self.address)
    }
}

impl<C: Interfaces> Address<C> {
    /// The IDs of the interfaces this address is typed by, in declaration
    /// order.
    pub const INTERFACE_IDS: &'static [crate::InterfaceId] = C::IDS;
}

/// The interfaces an [`Address`] is typed by: a generated client, or a tuple
/// of them.
pub trait Interfaces {
    /// The interface IDs, in declaration order.
    const IDS: &'static [crate::InterfaceId];
}

/// States that an [`Address`] typed by `Self` gives the client `D`.
///
/// `Self` is `D` itself, or a tuple of clients that contains `D`. `I` is the
/// position of `D`, and is always inferred. Listing one client twice makes
/// that inference ambiguous.
#[diagnostic::on_unimplemented(
    message = "this address is not typed by `{D}`",
    label = "`{D}` is not among the clients of `Address<{Self}>`",
    note = "add `{D}` to the address type, or retype the address with \
            `Address::cast`"
)]
pub trait Provides<D, I> {}

/// The position of a client that types an [`Address`] alone.
#[doc(hidden)]
pub struct Sole;

/// The position of a client in a tuple that types an [`Address`].
#[doc(hidden)]
pub struct At<const N: usize>;

impl<C: ContractClient> Interfaces for C {
    const IDS: &'static [crate::InterfaceId] = &[C::INTERFACE_ID];
}

impl<C: ContractClient> Provides<C, Sole> for C {}

macro_rules! impl_client_tuple {
    ($($all:ident),+ ; $($client:ident @ $n:literal),+) => {
        impl<$($all: ContractClient),+> Interfaces for ($($all,)+) {
            const IDS: &'static [crate::InterfaceId] =
                &[$($all::INTERFACE_ID),+];
        }

        impl_client_tuple!(@provides ($($all),+) $($client @ $n),+);
    };
    (@provides $all:tt $($client:ident @ $n:literal),+) => {
        $( impl_client_tuple!(@one $all $client $n); )+
    };
    (@one ($($all:ident),+) $client:ident $n:literal) => {
        impl<$($all),+> Provides<$client, At<$n>> for ($($all,)+) {}
    };
}

impl_client_tuple!(A, B; A @ 0, B @ 1);
impl_client_tuple!(A, B, C; A @ 0, B @ 1, C @ 2);
impl_client_tuple!(A, B, C, D; A @ 0, B @ 1, C @ 2, D @ 3);
impl_client_tuple!(A, B, C, D, E; A @ 0, B @ 1, C @ 2, D @ 3, E @ 4);
impl_client_tuple!(
    A, B, C, D, E, F;
    A @ 0, B @ 1, C @ 2, D @ 3, E @ 4, F @ 5
);

// The usual derives would bound `C`, which is only a marker.

impl<C> Clone for Address<C> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for Address<C> {}

impl<C> Default for Address<C> {
    fn default() -> Self {
        Self::new(ContractAddress::default())
    }
}

impl<C> fmt::Debug for Address<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.address, f)
    }
}

impl<C> PartialEq for Address<C> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

impl<C> Eq for Address<C> {}

impl<C> PartialEq<ContractAddress> for Address<C> {
    #[inline]
    fn eq(&self, other: &ContractAddress) -> bool {
        self.address == *other
    }
}

impl<C> PartialEq<Address<C>> for ContractAddress {
    #[inline]
    fn eq(&self, other: &Address<C>) -> bool {
        *self == other.address
    }
}

impl<C> PartialOrd for Address<C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<C> Ord for Address<C> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.address.cmp(&other.address)
    }
}

impl<C> Hash for Address<C> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address.hash(state);
    }
}

/// An `Address<C>` is usable wherever a `&ContractAddress` is expected.
impl<C> Deref for Address<C> {
    type Target = ContractAddress;

    #[inline]
    fn deref(&self) -> &ContractAddress {
        &self.address
    }
}

impl<C> AsRef<ContractAddress> for Address<C> {
    #[inline]
    fn as_ref(&self) -> &ContractAddress {
        &self.address
    }
}

impl<C> From<ContractAddress> for Address<C> {
    #[inline]
    fn from(address: ContractAddress) -> Self {
        Self::new(address)
    }
}

impl<C> From<&ContractAddress> for Address<C> {
    #[inline]
    fn from(address: &ContractAddress) -> Self {
        Self::new(*address)
    }
}

impl<'a, C> From<&'a ContractAddress> for &'a Address<C> {
    #[inline]
    fn from(address: &'a ContractAddress) -> Self {
        Address::from_ref(address)
    }
}

impl<C> From<Address<C>> for ContractAddress {
    #[inline]
    fn from(address: Address<C>) -> Self {
        address.address
    }
}

// ── Wire and storage form: that of `ContractAddress` ──

impl<'r, C> ReadXDR<'r> for Address<C> {
    #[inline]
    fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
        ContractAddress::read_xdr(r).map(Self::new)
    }
}

impl<'r: 'a, 'a, C> ReadXDR<'r> for &'a Address<C> {
    #[inline]
    fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
        <&'a ContractAddress>::read_xdr(r).map(Address::from_ref)
    }
}

impl<C> WriteXDR for Address<C> {
    #[inline]
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        self.address.write_xdr(w)
    }

    #[inline]
    unsafe fn write_xdr_unchecked<F, W: WriterUnchecked<F>>(&self, w: &mut W) {
        // SAFETY: the caller's guarantees for `self` are those for the
        // address, which is all that is written.
        unsafe { self.address.write_xdr_unchecked(w) }
    }
}

impl<C> cxs::ConstMaxSize for Address<C> {
    const MAX_SIZE: Option<usize> =
        <ContractAddress as cxs::ConstMaxSize>::MAX_SIZE;
}

impl<C> txs::ConstMaxSize for Address<C> {
    const MAX_SIZE: Option<usize> =
        <ContractAddress as txs::ConstMaxSize>::MAX_SIZE;
}

/// The spec type is a plain address: the spec format has no node for the
/// expected interface.
impl<C> thruput_contract_spec::ContractTypeRefSpec for Address<C> {
    const INFO: thruput_contract_spec::ContractTypeInfo<'static> =
        <ContractAddress as thruput_contract_spec::ContractTypeRefSpec>::INFO;
}

// ── Contract parameters, auth tokens and events: as `ContractAddress` ──

impl<'r, C: 'r> RefArg<'r> for Address<C> {
    type Decoded = &'r Address<C>;

    #[inline(always)]
    fn get(decoded: &Self::Decoded) -> &Self {
        decoded
    }
}

impl<C> RefArgElement for Address<C> {}

impl<C> CollectAuthHandles for Address<C> {
    #[inline(always)]
    fn collect_auth_handles(
        &self,
        _out: &mut AuthHandleBuf,
    ) -> Result<(), FaultString> {
        Ok(())
    }
}

impl<C> ReceiveAuthHandles for Address<C> {
    #[inline(always)]
    fn receive_auth_handles(
        &mut self,
        _handles: &mut impl Iterator<Item = u32>,
    ) -> Result<(), AuthTokenError> {
        Ok(())
    }
}

impl<C> MintableType for Address<C> {
    type Preimage = Address<C>;
}

impl<C> Mintable for Address<C> {
    fn mint_into_parent(
        preimage: Address<C>,
        _children: &mut ChildNodeBuf,
    ) -> Self {
        preimage
    }
}

impl<C> IntoTopic for Address<C> {
    fn into_topic(&self) -> TopicSrc<'_> {
        self.address.into_topic()
    }
}
