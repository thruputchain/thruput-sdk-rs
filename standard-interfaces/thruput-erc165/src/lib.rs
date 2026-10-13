//! ERC-165 for Thruput: a contract answers "do you implement interface
//! X?" for an [`InterfaceId`].
//!
//! On Ethereum an interface id is the XOR of the interface's function
//! selectors and `supportsInterface` is a hand-maintained claim. Here every
//! `#[contract]` trait already carries an `INTERFACE_ID` derived from its name
//! and major version — the pair that namespaces its entrypoint IDs — and every
//! `#[contract]` impl block leaves a record of that id in the contract's
//! interface table (see `thruput_sdk::interface_table`). So the claim is not
//! written at all: [`implements!`] reads it off the impls that actually
//! linked.
//!
//! ```ignore
//! thruput_erc165_if::impl_erc165!();
//! ```
//!
//! That expands to a `#[contract] impl ERC165` on a hidden stateless
//! struct whose `supports_interface` is `implements!(id)` and whose
//! `supports_interfaces` answers a list of ids the same way, one call for
//! all of them; the methods take no `self`, so no storage struct is
//! involved. Contracts that do not invoke it carry neither entrypoint and
//! no records in their image: the table is anchored only by the reader
//! behind [`implements!`], so it is dropped from any image that never
//! calls it.
//!
//! The Ethereum detection dance (a 30k-gas `staticcall`, then probing
//! `0xffffffff`) is unnecessary: a contract without this interface throws
//! the typed `LibraryExn::UnknownMethod`, which
//! [`ERC165Client::supports`] reads as "unsupported", and there is no
//! fallback function to lie.
//!
//! The claim remains self-reported in the sense that it describes this
//! contract's own code: `supports_interface` says "I export these
//! entrypoints under this interface's spec", nothing more. Treat it as a
//! dispatch hint, not an authorization boundary.

#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

use thruput_sdk::contract;

pub use thruput_sdk::InterfaceId;
pub use thruput_sdk::implements;

/// Upper bound on the number of interface ids one [`ERC165::supports_interfaces`]
/// query carries.
///
/// A query is a bounded list so that the contract decodes it with no
/// allocator; a caller with more ids than this makes more than one call.
pub const INTERFACE_QUERY_MAX: usize = 32;

/// The interface ids one [`ERC165::supports_interfaces`] query asks about.
pub type InterfaceIds = heapless::Vec<InterfaceId, INTERFACE_QUERY_MAX>;

/// The answers to one [`ERC165::supports_interfaces`] query: one per
/// queried id, in the query's order.
pub type InterfaceSupport = heapless::Vec<bool, INTERFACE_QUERY_MAX>;

/// The ERC-165 interface: query whether a contract implements an interface,
/// one at a time or as a list.
///
/// `supports_interface` returns `true` for [`ERC165::INTERFACE_ID`] itself,
/// since the impl block that provides it is one more record in the table.
/// `supports_interfaces` answers a list of ids in one call, one answer per
/// id in the query's order. Neither takes `self`, so callers may invoke them
/// read-only.
#[contract(client = ERC165Client)]
pub trait ERC165 {
    /// True if this contract implements the interface identified by
    /// `interface_id`.
    fn supports_interface(interface_id: InterfaceId) -> bool;

    /// For each id in `interface_ids`, in order, whether this contract
    /// implements the interface it identifies.
    ///
    /// Equivalent to calling [`supports_interface`](ERC165::supports_interface)
    /// once per id: an id that appears twice is answered twice, and an
    /// empty query gets an empty answer.
    fn supports_interfaces(interface_ids: &InterfaceIds) -> InterfaceSupport;
}

/// Give this contract ERC-165: a `#[contract] impl ERC165` whose
/// `supports_interface` is [`implements!`], and whose `supports_interfaces`
/// is [`implements!`] per queried id, on a hidden stateless carrier struct.
///
/// ```ignore
/// thruput_erc165_if::impl_erc165!();
/// ```
///
/// Invoke it once, at module level in the contract crate, next to the
/// contract's other `#[contract]` impls. The methods take no `self`, so the
/// impl is not tied to the contract's storage struct; the carrier is
/// stateless and registers with the host mock like a free function. Writing
/// the same block by hand on any type is equivalent.
#[macro_export]
macro_rules! impl_erc165 {
    () => {
        /// Stateless carrier for this contract's ERC-165 impl.
        #[doc(hidden)]
        pub struct __ThruputErc165;

        #[::thruput_sdk::contract]
        impl $crate::ERC165 for __ThruputErc165 {
            fn supports_interface(interface_id: $crate::InterfaceId) -> bool {
                $crate::implements!(interface_id)
            }

            fn supports_interfaces(
                interface_ids: &$crate::InterfaceIds,
            ) -> $crate::InterfaceSupport {
                // The answer shares the query's bound, so collecting one
                // answer per queried id cannot overflow it.
                interface_ids
                    .iter()
                    .map(|id| $crate::implements!(*id))
                    .collect()
            }
        }
    };
}

/// A set of interfaces to ask one contract about in a single
/// [`ERC165Client::supports`] call: a tuple of one to eight of their
/// generated clients, such as `(ERC721Client, ERC721MetadataClient)`.
pub trait InterfaceQuery: sealed::Sealed {
    /// One answer per interface, in tuple order.
    type Answer;

    #[doc(hidden)]
    const IDS: &'static [InterfaceId];

    #[doc(hidden)]
    fn answer(support: &[bool]) -> Self::Answer;
}

mod sealed {
    pub trait Sealed {}
}

macro_rules! interface_query {
    ($n:literal: $($client:ident),+) => {
        impl<$($client: thruput_sdk::ContractClient),+> sealed::Sealed
            for ($($client,)+)
        {
        }

        impl<$($client: thruput_sdk::ContractClient),+> InterfaceQuery
            for ($($client,)+)
        {
            type Answer = [bool; $n];

            const IDS: &'static [InterfaceId] =
                &[$($client::INTERFACE_ID),+];

            fn answer(support: &[bool]) -> [bool; $n] {
                core::array::from_fn(|i| support.get(i) == Some(&true))
            }
        }
    };
}

interface_query!(1: A);
interface_query!(2: A, B);
interface_query!(3: A, B, C);
interface_query!(4: A, B, C, D);
interface_query!(5: A, B, C, D, E);
interface_query!(6: A, B, C, D, E, F);
interface_query!(7: A, B, C, D, E, F, G);
interface_query!(8: A, B, C, D, E, F, G, H);

impl ERC165Client {
    /// Whether the target implements each interface in `Q`, a tuple of
    /// interface clients, from one read-only `supports_interfaces` call. A
    /// failed call or an unanswered id counts as unsupported.
    pub fn supports<Q: InterfaceQuery>(&self) -> Q::Answer {
        let support = self
            .with_read_only()
            .try_supports_interfaces(&query_ids::<Q>())
            .unwrap_or_default();
        Q::answer(&support)
    }
}

/// `Q`'s ids as a query. At most eight, so they always fit.
fn query_ids<Q: InterfaceQuery>() -> InterfaceIds {
    Q::IDS.iter().copied().collect()
}
