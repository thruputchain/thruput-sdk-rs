//! Invoking a deployed contract from off-chain code.
//!
//! With the `thruput-sdk/remote` feature, every `#[contract]` interface also
//! generates a remote proxy (`<Iface>Remote<I>`) next to its guest client.
//! The proxy has one method per interface method. Each method encodes its
//! arguments the way the guest client does and passes them, with the method's
//! [`RemoteMethod`] identity, to a caller-supplied [`RemoteInvoker`]. The
//! invoker decides what a prepared invocation is: a transaction to sign and
//! submit, a simulation, or a test harness call.

use thruput_xdr::contract::ContractEntrypointID;
use thruput_xdr::types::ContractAddress;
use xdrs::{ReadXDROwned, WriteXDR};

/// A method's declared effect, from `#[contract(effect = "...")]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodEffect {
    /// Uses only arguments and constants.
    Pure,
    /// Reads state or execution context, without application effects.
    ReadOnly,
    /// May change storage, emit events, transfer, or deploy.
    StateChanging,
}

/// The identity of one interface method, fixed when its interface compiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteMethod {
    /// Interface name, or `None` for an unnamed interface.
    pub interface: Option<&'static str>,
    /// Interface version the entrypoint was derived from.
    pub version: &'static str,
    /// Method name.
    pub name: &'static str,
    /// Entrypoint selector.
    pub entrypoint: ContractEntrypointID,
    /// Declared effect, or `None` when the interface declares none.
    pub effect: Option<MethodEffect>,
}

impl RemoteMethod {
    /// Whether the declaration permits simulating instead of submitting.
    /// An undeclared effect is never treated as read-only.
    pub fn is_read_only(&self) -> bool {
        matches!(
            self.effect,
            Some(MethodEffect::Pure | MethodEffect::ReadOnly)
        )
    }
}

/// Prepares invocations of deployed contracts for a remote proxy.
///
/// The proxy routes `pure` and `readOnly` methods to [`view`](Self::view)
/// and every other method, including those without a declared effect, to
/// [`call`](Self::call). `args` holds the method's arguments: the single
/// argument itself, or a tuple of several. Implementations encode it before
/// returning, since the prepared invocation cannot borrow it.
pub trait RemoteInvoker {
    /// A prepared invocation of a method that may change state.
    type Call<R>;
    /// A prepared read of a `pure` or `readOnly` method.
    type View<R>;

    /// Prepare an invocation of `method` on `target`.
    fn call<R, A>(
        &self,
        target: &ContractAddress,
        method: RemoteMethod,
        args: &A,
    ) -> Self::Call<R>
    where
        R: ReadXDROwned,
        A: WriteXDR + ?Sized;

    /// Prepare a read of `method` on `target`.
    fn view<R, A>(
        &self,
        target: &ContractAddress,
        method: RemoteMethod,
        args: &A,
    ) -> Self::View<R>
    where
        R: ReadXDROwned,
        A: WriteXDR + ?Sized;
}
