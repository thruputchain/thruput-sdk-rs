//! An implementation of ERC721 with metadata.
//!
//! Storage is laid out so that unrelated transfers never contend:
//!
//! - `owners` and `token_approvals` are keyed by token id, so two transfers
//!   touch the same slot only when they move the same token, which must
//!   serialize anyway.
//! - `balances` are [`NNInt`] counters adjusted with lazy increments, which
//!   merge: an owner sending two tokens in one block, or receiving one while
//!   sending another, produces two deltas, not a conflict.
//! - `operator_approvals` are set or deleted, never read-modify-written.
//!
//! Every slot is written under an exclusive lock. Two transactions in one
//! block execute against the same prior state, so two transfers of one
//! token to the same recipient would agree on the new owner and, with
//! plain writes, both commit, moving the balances twice. Exclusive writes
//! let one of them land and fail the other at commit.
//!
//! Metadata is per token, chosen at mint: a URI, or the fields on chain.
//!
//! Minting is restricted to the administrator named at `initialize`, as in
//! the `erc20` example; burning is open to anyone the token's owner has
//! authorized, as in OpenZeppelin's `ERC721Burnable`.

use crate::interface::*;
use sdk::call::CallError;
use sdk::storage::{Deletable, Map, NNInt, Value};
use sdk::*;

#[sdk::storage]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct NftContract {
    config: Value<Config>,
    owners: Map<TokenId, Value<ContractAddress>>,
    balances: Map<ContractAddress, NNInt>,
    token_approvals: Map<TokenId, Value<ContractAddress>>,
    operator_approvals:
        Map<ContractAddress, Map<ContractAddress, Value<bool>>>,
    metadata: Map<TokenId, Value<TokenMetadata>>,
}

/// Errors of this contract's own administrative surface, beyond the
/// standard [`ERC721Error`]s.
#[contract_exception]
#[non_exhaustive]
pub enum NftError {
    AlreadyInitialized = 1,

    MustBeAdministrator = 2,

    TokenAlreadyMinted = 3,
}

#[contract_type]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct Config {
    pub administrator: ContractAddress,
    pub name: Name,
    pub symbol: Symbol,
}

impl NftContract {
    fn _administrator(&self) -> ContractAddress {
        self.config.get().administrator
    }

    /// Owner of `token_id`, or `None` if it does not exist.
    fn _owner_of(&self, token_id: &TokenId) -> Option<ContractAddress> {
        self.owners.at(token_id).try_get().none_if_missing()
    }

    /// Owner of `token_id`, throwing [`ERC721Error::NonexistentToken`].
    fn _require_owned(&self, token_id: &TokenId) -> ContractAddress {
        self._owner_of(token_id)
            .unwrap_or_else(|| throw(ERC721Error::NonexistentToken))
    }

    fn _approved_for(&self, token_id: &TokenId) -> ContractAddress {
        self.token_approvals
            .at(token_id)
            .try_get()
            .none_if_missing()
            .unwrap_or(*CONTRACT_ADDR_ZERO)
    }

    fn _is_operator(
        &self,
        owner: &ContractAddress,
        operator: &ContractAddress,
    ) -> bool {
        self.operator_approvals
            .at(owner)
            .at(operator)
            .try_get()
            .none_if_missing()
            .unwrap_or(false)
    }

    /// True when `spender` may move `token_id`, currently owned by `owner`.
    fn _is_authorized(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
        token_id: &TokenId,
    ) -> bool {
        spender == owner
            || self._is_operator(owner, spender)
            || self._approved_for(token_id) == *spender
    }

    /// Move `token_id` from `from` to `to` on behalf of `spender`, with the
    /// standard checks, and emit [`TransferEvent`].
    fn _transfer(
        &mut self,
        spender: &ContractAddress,
        from: &ContractAddress,
        to: &ContractAddress,
        token_id: &TokenId,
    ) {
        let owner = self._require_owned(token_id);
        ensure(
            self._is_authorized(&owner, spender, token_id),
            ERC721Error::InsufficientApproval,
        );
        ensure(owner == *from, ERC721Error::IncorrectOwner);

        self.token_approvals.at_mut_exclusive(token_id).delete();
        self.balances.at_mut(from).inc(-1);
        self.balances.at_mut(to).inc(1);
        self.owners.at_mut_exclusive(token_id).set(to);

        log_transfer_event(from, to, token_id);
    }

    /// Hand `token_id` to `to`'s receiver hook after a transfer, mapping a
    /// destination without one to [`ERC721Error::InvalidReceiver`] and
    /// re-throwing anything the hook itself threw.
    fn _check_on_erc721_received(
        operator: &ContractAddress,
        from: &ContractAddress,
        to: &ContractAddress,
        token_id: &TokenId,
        data: TransferData,
    ) {
        match ERC721ReceiverClient::new(to)
            .try_on_erc721_received(operator, from, token_id, data)
        {
            Ok(()) => {}
            // No contract at `to`, or one without the hook.
            Err(CallError::Invalid) => throw(ERC721Error::InvalidReceiver),
            Err(err) if err.try_as() == Some(LibraryExn::UnknownMethod) => {
                throw(ERC721Error::InvalidReceiver)
            }
            // The receiver rejected the token: its reason reaches the caller.
            Err(err) => err.re_throw(),
        }
    }
}

// The ERC721 core. Every method takes the caller from the frame, never
// from an argument, so authorization cannot be spoofed.
#[contract]
impl ERC721 for NftContract {
    fn balance_of(&self, owner: &ContractAddress) -> u64 {
        self.balances
            .at(owner)
            .try_get()
            .none_if_missing()
            .unwrap_or(0)
            .try_into()
            .unwrap_or_else(|_| throw(LibraryExn::NonNegativeIntUnderflow))
    }

    fn owner_of(&self, token_id: &TokenId) -> ContractAddress {
        self._require_owned(token_id)
    }

    fn safe_transfer_from(
        &mut self,
        from: &ContractAddress,
        to: &ContractAddress,
        token_id: &TokenId,
        data: TransferData,
    ) {
        let operator = get_caller();
        self._transfer(&operator, from, to, token_id);
        Self::_check_on_erc721_received(&operator, from, to, token_id, data);
    }

    fn transfer_from(
        &mut self,
        from: &ContractAddress,
        to: &ContractAddress,
        token_id: &TokenId,
    ) {
        let spender = get_caller();
        self._transfer(&spender, from, to, token_id);
    }

    fn approve(&mut self, to: &ContractAddress, token_id: &TokenId) {
        let caller = get_caller();
        let owner = self._require_owned(token_id);
        ensure(
            caller == owner || self._is_operator(&owner, &caller),
            ERC721Error::InvalidApprover,
        );

        if to == CONTRACT_ADDR_ZERO {
            self.token_approvals.at_mut_exclusive(token_id).delete();
        } else {
            self.token_approvals.at_mut_exclusive(token_id).set(to);
        }

        log_approval_event(&owner, to, token_id);
    }

    fn set_approval_for_all(
        &mut self,
        operator: &ContractAddress,
        approved: bool,
    ) {
        let owner = get_caller();

        let mut approvals = self.operator_approvals.at_mut(&owner);
        let mut slot = approvals.at_mut_exclusive(operator);
        if approved {
            slot.set(&true);
        } else {
            slot.delete();
        }

        log_approval_for_all_event(&owner, operator, approved);
    }

    fn get_approved(&self, token_id: &TokenId) -> ContractAddress {
        self._require_owned(token_id);
        self._approved_for(token_id)
    }

    fn is_approved_for_all(
        &self,
        owner: &ContractAddress,
        operator: &ContractAddress,
    ) -> bool {
        self._is_operator(owner, operator)
    }
}

#[contract]
impl ERC721Metadata for NftContract {
    fn name(&self) -> Name {
        self.config.get().name
    }

    fn symbol(&self) -> Symbol {
        self.config.get().symbol
    }

    fn token_uri(&self, token_id: &TokenId) -> TokenUri {
        match self.token_metadata(token_id) {
            TokenMetadata::Uri(uri) => uri,
            TokenMetadata::Onchain(_) => throw(ERC721Error::NoTokenUri),
        }
    }

    fn token_metadata(&self, token_id: &TokenId) -> TokenMetadata {
        self._require_owned(token_id);
        self.metadata.at(token_id).get()
    }
}

/// This contract's own surface: setup, minting, and burning.
#[contract(client = NftAdminClient, throws(NftError, ERC721Error))]
impl NftContract {
    /// Name the administrator and the collection. Throws
    /// [`NftError::AlreadyInitialized`] if called more than once.
    ///
    /// Call this in the transaction that deploys the contract, as the
    /// tests do: the first caller names the administrator, so a contract
    /// left uninitialized across a block boundary is anyone's to claim.
    /// A Groundhog transaction is atomic, so deploy-and-initialize in one
    /// leaves no such window.
    pub fn initialize(
        &mut self,
        administrator: &ContractAddress,
        name: Name,
        symbol: Symbol,
    ) {
        if self.config.try_get().none_if_missing().is_some() {
            throw(NftError::AlreadyInitialized);
        }
        self.config.set_exclusive(&Config {
            administrator: *administrator,
            name,
            symbol,
        });
    }

    /// Create `token_id` owned by `to`, with `metadata` at a URI or on
    /// chain, and emit a [`MintEvent`]. Throws
    /// [`NftError::MustBeAdministrator`] unless the caller is the
    /// administrator, and [`NftError::TokenAlreadyMinted`] if the id
    /// exists.
    pub fn mint(
        &mut self,
        to: &ContractAddress,
        token_id: &TokenId,
        metadata: TokenMetadata,
    ) {
        ensure(
            get_caller() == self._administrator(),
            NftError::MustBeAdministrator,
        );
        ensure(
            self._owner_of(token_id).is_none(),
            NftError::TokenAlreadyMinted,
        );

        self.balances.at_mut(to).inc(1);
        self.owners.at_mut_exclusive(token_id).set(to);
        self.metadata.at_mut_exclusive(token_id).set(&metadata);

        log_mint_event(to, token_id);
    }

    /// Destroy `token_id` and emit a [`BurnEvent`].
    /// The caller needs the same authorization as
    /// for a transfer; throws [`ERC721Error::NonexistentToken`] and
    /// [`ERC721Error::InsufficientApproval`].
    pub fn burn(&mut self, token_id: &TokenId) {
        let caller = get_caller();
        let owner = self._require_owned(token_id);
        ensure(
            self._is_authorized(&owner, &caller, token_id),
            ERC721Error::InsufficientApproval,
        );

        self.token_approvals.at_mut_exclusive(token_id).delete();
        self.balances.at_mut(&owner).inc(-1);
        self.owners.at_mut_exclusive(token_id).delete();
        self.metadata.at_mut_exclusive(token_id).delete();

        log_burn_event(&owner, token_id);
    }
}

// ERC-165 interface detection over the impl blocks above.
groundhog_erc165_if::impl_erc165!();
