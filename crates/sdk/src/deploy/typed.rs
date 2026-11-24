//! [`CodeHash<C>`] and [`DeployNonce`]: the two inputs of a deployment, each
//! with its own type.

use core::cmp::Ordering;
#[cfg(not(target_arch = "riscv32"))]
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

use crate::call::{Address, RefArg, RefArgElement};
use crate::collect_auth_handles::{AuthHandleBuf, CollectAuthHandles};
use crate::event::{IntoTopic, TopicSrc};
use crate::mintable::{ChildNodeBuf, Mintable, MintableType};
use crate::receive_auth_handles::ReceiveAuthHandles;
use crate::types::{ContractAddress, SHA256Hash};
use crate::xdr::{
    ReadResult, ReadXDR, Reader, WriteResult, WriteXDR, Writer,
    WriterUnchecked, cxs, txs,
};
use crate::{AuthTokenError, FaultString};

/// The hash of a contract's code, typed by the client, or tuple of clients,
/// used to call a contract deployed from it. It encodes exactly as the
/// untyped [`SHA256Hash`].
///
/// [`deploy`](Self::deploy) returns an [`Address<C>`]. The type records the
/// expected interfaces and does not check them, and it does not convert to a
/// `&SHA256Hash` implicitly; see [Deployment](crate#deployment). `Debug` is
/// implemented on host targets only.
#[repr(transparent)]
pub struct CodeHash<C> {
    hash: SHA256Hash,
    // `fn() -> C` keeps `CodeHash<C>` `Send`, `Sync` and `Copy` whatever `C`
    // is: no `C` is ever held.
    client: PhantomData<fn() -> C>,
}

impl<C> CodeHash<C> {
    /// Type `hash` as the code of a contract called through `C`.
    pub const fn new(hash: SHA256Hash) -> Self {
        Self {
            hash,
            client: PhantomData,
        }
    }

    /// Type a borrowed `hash` as the code of a contract called through `C`.
    pub const fn from_ref(hash: &SHA256Hash) -> &Self {
        // SAFETY: `CodeHash<C>` is `repr(transparent)` over `SHA256Hash`.
        unsafe { &*(hash as *const SHA256Hash as *const Self) }
    }

    /// The untyped hash.
    pub const fn as_hash(&self) -> &SHA256Hash {
        &self.hash
    }

    /// The untyped hash, by value.
    pub const fn into_hash(self) -> SHA256Hash {
        self.hash
    }

    /// The same hash, retyped without a check.
    pub const fn cast<D>(self) -> CodeHash<D> {
        CodeHash::new(self.hash)
    }

    /// Deploy a contract with this code from the current contract, and
    /// return its address.
    ///
    /// The address is a function of the current contract's address, `nonce`
    /// and this hash, so each nonce deploys this code once.
    #[inline]
    pub fn deploy(&self, nonce: &DeployNonce) -> Address<C> {
        Address::new(super::deploy_contract(&nonce.0, &self.hash))
    }

    /// The address [`deploy`](Self::deploy) gives for `nonce` when the
    /// current contract calls it. Nothing is deployed.
    #[inline]
    pub fn deploy_address(&self, nonce: &DeployNonce) -> Address<C> {
        Address::new(super::get_deploy_address(&nonce.0, &self.hash))
    }

    /// The address [`deploy`](Self::deploy) gives for `nonce` when
    /// `deployer` calls it. Nothing is deployed.
    #[inline]
    pub fn deploy_address_for(
        &self,
        deployer: &ContractAddress,
        nonce: &DeployNonce,
    ) -> Address<C> {
        Address::new(super::get_deploy_address_for_other(
            deployer, &nonce.0, &self.hash,
        ))
    }

    /// The code hash of a mock contract registered in a test environment,
    /// typed as a contract called through `C`.
    #[cfg(feature = "dev")]
    pub fn of_mock(mock: &crate::MockContract) -> Self {
        Self::new(*mock.as_hash())
    }
}

/// The nonce of a deployment: with the deployer's address and the code hash,
/// it determines the address of the new contract. It encodes exactly as the
/// untyped [`SHA256Hash`].
///
/// A deployer uses each nonce once per code hash. The type says what the
/// hash is for and checks nothing about it, and it does not convert to a
/// `&SHA256Hash` implicitly; see [Deployment](crate#deployment). `Debug` is
/// implemented on host targets only.
#[repr(transparent)]
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeployNonce(SHA256Hash);

impl DeployNonce {
    /// Use `hash` as a deploy nonce.
    pub const fn new(hash: SHA256Hash) -> Self {
        Self(hash)
    }

    /// Use a borrowed `hash` as a deploy nonce.
    pub const fn from_ref(hash: &SHA256Hash) -> &Self {
        // SAFETY: `DeployNonce` is `repr(transparent)` over `SHA256Hash`.
        unsafe { &*(hash as *const SHA256Hash as *const Self) }
    }

    /// The nonce that is the SHA-256 of the encoding of `preimage`, as
    /// [`Hash::hash`](crate::Hash::hash) computes it.
    ///
    /// `preimage` is usually a tuple. Start it with a constant that no
    /// other hash in the contract starts with, and include every value the
    /// address must depend on.
    #[inline]
    pub fn derive<T: WriteXDR>(preimage: &T) -> Self {
        Self(crate::Hash::hash(preimage))
    }

    /// The untyped hash.
    pub const fn as_hash(&self) -> &SHA256Hash {
        &self.0
    }

    /// The untyped hash, by value.
    pub const fn into_hash(self) -> SHA256Hash {
        self.0
    }
}

#[cfg(not(target_arch = "riscv32"))]
impl fmt::Debug for DeployNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

// The usual derives would bound `C`, which is only a marker.

impl<C> Clone for CodeHash<C> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for CodeHash<C> {}

impl<C> Default for CodeHash<C> {
    fn default() -> Self {
        Self::new(SHA256Hash::default())
    }
}

#[cfg(not(target_arch = "riscv32"))]
impl<C> fmt::Debug for CodeHash<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.hash, f)
    }
}

impl<C> PartialEq for CodeHash<C> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl<C> Eq for CodeHash<C> {}

impl<C> PartialOrd for CodeHash<C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<C> Ord for CodeHash<C> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.hash.cmp(&other.hash)
    }
}

impl<C> Hash for CodeHash<C> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&self.hash, state);
    }
}

/// The impls that make a transparent wrapper of [`SHA256Hash`] encode, store
/// and pass as one. `$ty` has a `from_ref` and converts with `$get`.
macro_rules! encodes_as_sha256_hash {
    ([$($g:ident)?] $ty:ty, |$this:ident| $get:expr) => {
        impl<$($g)?> PartialEq<SHA256Hash> for $ty {
            #[inline]
            fn eq(&self, other: &SHA256Hash) -> bool {
                let $this = self;
                $get == other
            }
        }

        impl<$($g)?> PartialEq<$ty> for SHA256Hash {
            #[inline]
            fn eq(&self, other: &$ty) -> bool {
                let $this = other;
                self == $get
            }
        }

        impl<$($g)?> From<$ty> for SHA256Hash {
            #[inline]
            fn from($this: $ty) -> Self {
                *$get
            }
        }

        // The wire and storage form is that of `SHA256Hash`.

        impl<'r, $($g)?> ReadXDR<'r> for $ty {
            #[inline]
            fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
                SHA256Hash::read_xdr(r).map(Self::new)
            }
        }

        impl<'r: 'a, 'a, $($g)?> ReadXDR<'r> for &'a $ty {
            #[inline]
            fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
                <&'a SHA256Hash>::read_xdr(r).map(<$ty>::from_ref)
            }
        }

        impl<$($g)?> WriteXDR for $ty {
            #[inline]
            fn write_xdr<F, W: Writer<F>>(
                &self,
                w: &mut W,
            ) -> WriteResult<()> {
                let $this = self;
                $get.write_xdr(w)
            }

            #[inline]
            unsafe fn write_xdr_unchecked<F, W: WriterUnchecked<F>>(
                &self,
                w: &mut W,
            ) {
                let $this = self;
                // SAFETY: the caller's guarantees for `self` are those for
                // the hash, which is all that is written.
                unsafe { $get.write_xdr_unchecked(w) }
            }
        }

        impl<$($g)?> cxs::ConstMaxSize for $ty {
            const MAX_SIZE: Option<usize> =
                <SHA256Hash as cxs::ConstMaxSize>::MAX_SIZE;
        }

        impl<$($g)?> txs::ConstMaxSize for $ty {
            const MAX_SIZE: Option<usize> =
                <SHA256Hash as txs::ConstMaxSize>::MAX_SIZE;
        }

        /// The spec type is a plain hash: the spec format has no node for
        /// what the hash is of.
        impl<$($g)?> thruput_contract_spec::ContractTypeRefSpec for $ty {
            const INFO: thruput_contract_spec::ContractTypeInfo<'static> =
                <SHA256Hash as thruput_contract_spec::ContractTypeRefSpec>::INFO;
        }

        // Contract parameters, auth tokens and events treat it as a
        // `SHA256Hash`.

        impl<'r, $($g: 'r)?> RefArg<'r> for $ty {
            type Decoded = &'r $ty;

            #[inline(always)]
            fn get(decoded: &Self::Decoded) -> &Self {
                decoded
            }
        }

        impl<$($g)?> RefArgElement for $ty {}

        impl<$($g)?> CollectAuthHandles for $ty {
            #[inline(always)]
            fn collect_auth_handles(
                &self,
                _out: &mut AuthHandleBuf,
            ) -> Result<(), FaultString> {
                Ok(())
            }
        }

        impl<$($g)?> ReceiveAuthHandles for $ty {
            #[inline(always)]
            fn receive_auth_handles(
                &mut self,
                _handles: &mut impl Iterator<Item = u32>,
            ) -> Result<(), AuthTokenError> {
                Ok(())
            }
        }

        impl<$($g)?> MintableType for $ty {
            type Preimage = $ty;
        }

        impl<$($g)?> Mintable for $ty {
            fn mint_into_parent(
                preimage: $ty,
                _children: &mut ChildNodeBuf,
            ) -> Self {
                preimage
            }
        }

        impl<$($g)?> IntoTopic for $ty {
            fn into_topic(&self) -> TopicSrc<'_> {
                let $this = self;
                $get.into_topic()
            }
        }
    };
}

encodes_as_sha256_hash!([C] CodeHash<C>, |this| &this.hash);
encodes_as_sha256_hash!([] DeployNonce, |this| &this.0);
