//! Routines for reading transaction AuthClaims.
//!
//! These routines are thin wrappers around VM syscalls.
// TODO(geoff): Think about the following questions:
// 1: is there a better interface in which to wrap these calls?
// 2: Do we want some runtime-checked "single-use" checks for these authclaims?

use crate::internal::MUBuf;
use crate::sys::{self, SysResult};
use crate::types::RuntimeMemoryLength;
use core::mem::MaybeUninit;
use groundhog_xdr::auth_claim::AuthClaim;
use xdrs::ReadXDR;
use xdrs::txs::ConstMaxSize;

pub use groundhog_xdr::address_auth::{
    AddressAuthSummary, PublicKeyConfigSummary, PublicKeyConfigSummaryOpt,
};
pub use groundhog_xdr::auth_claim::BitMap;
pub use groundhog_xdr::auth_claim::Weight;
use groundhog_xdr::syscall_error::SyscallErrorCode;
use groundhog_xdr::types::{CryptographicScheme, Ed25519PublicKey, PublicKey};

pub fn get_auth_claim() -> SysResult<AuthClaim> {
    let mut auth_claim = MaybeUninit::uninit();
    sys::uninit_GetAuthClaim(&mut auth_claim)?;
    Ok(unsafe { auth_claim.assume_init() })
}

pub fn set_address_required_weight(required_weight: Weight) -> SysResult<()> {
    sys::uninit_WriteAddressRequiredWeight(required_weight)
}

fn key_bytes(public_key: &PublicKey) -> SysResult<&[u8]> {
    match public_key {
        PublicKey::Ed25519 { ed25519 } => Ok(ed25519.as_ref()),
        _ => {
            Err(SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.into())
        }
    }
}

pub fn add_address_public_key(
    pk_index: u32,
    public_key: PublicKey,
    key_weight: Weight,
) -> SysResult<()> {
    let scheme = public_key.scheme();
    sys::uninit_ChangePublicKeyRegistration(
        pk_index,
        Some(&scheme.0),
        key_bytes(&public_key)?,
        Some(&key_weight),
    )
}

pub fn remove_address_public_key(pk_index: u32) -> SysResult<()> {
    sys::uninit_ChangePublicKeyRegistration(pk_index, None, &[], None)
}

/// Encoded size of the largest `AddressAuthSummary` (every slot occupied):
/// fixed whatever key schemes are in use, since the summary carries no keys.
const ADDRESS_AUTH_SUMMARY_MAX_LEN: usize =
    match <AddressAuthSummary as ConstMaxSize>::MAX_SIZE {
        Some(len) => len,
        None => panic!("AddressAuthSummary has a finite encoded size"),
    };

/// Upper bound on the raw bytes of a registered key of any scheme: the XDR
/// bound of the `PublicKey` union, which covers every scheme's key bytes
/// (plus the union's tag).
const PUBLIC_KEY_MAX_LEN: usize = match <PublicKey as ConstMaxSize>::MAX_SIZE {
    Some(len) => len,
    None => panic!("PublicKey has a finite encoded size"),
};

/// Reads a summary of the executing contract's own key registration: for
/// each of the `kMaxSigners` slots whether it is occupied and, if so, the
/// key's scheme and weight, plus the required weight and the height at which
/// the row was last written. The keys themselves are read one slot at a time
/// with [`get_address_public_key`], so a scheme with large keys never forces
/// a contract to receive the whole table.
///
/// The summary is the current call frame's view: the committed row with this
/// transaction's pending [`add_address_public_key`],
/// [`remove_address_public_key`] and [`set_address_required_weight`] calls
/// applied, the same way storage reads see same-transaction writes; while
/// such a write is pending, `last_written_height` is the executing block
/// height. An address with no registration row reads as every slot empty
/// with `required_weight` 0, not as an error.
///
/// Admission checks committed state only: a key registered in this
/// transaction cannot authorize anything until the block carrying it lands.
pub fn get_address_auth_summary() -> SysResult<AddressAuthSummary> {
    let mut buffer: MUBuf<ADDRESS_AUTH_SUMMARY_MAX_LEN> = MUBuf::new();
    let mut written: MaybeUninit<RuntimeMemoryLength> = MaybeUninit::uninit();
    sys::uninit_GetAddressAuthSummary(
        buffer.spare_capacity_mut(),
        &mut written,
    )?;

    // The host reports the full encoded length and copies at most the
    // buffer, which holds every encoding; a longer report means the host
    // broke the syscall contract, and trusting it would read uninitialized
    // bytes.
    let written = unsafe { written.assume_init() } as usize;
    if written > buffer.capacity() {
        return Err(
            SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.into()
        );
    }
    // SAFETY: the host initialized the first `written` bytes.
    unsafe { buffer.set_len(written) };

    let mut reader = xdrs::txs::buf_reader::BufReader::new(buffer.as_slice());
    AddressAuthSummary::read_xdr(&mut reader).map_err(|_| {
        SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.into()
    })
}

/// Reads the key registered in slot `key_index` of the executing contract's
/// own registration, from the same view [`get_address_auth_summary`]
/// reports. The host returns the key's raw bytes; `scheme` says how to read
/// them, and comes from the summary's entry for the slot.
///
/// Returns [`SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist`] for an
/// empty slot and
/// [`SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput`] for a
/// `key_index` past `kMaxSigners`, for a scheme this SDK cannot decode, or
/// for a key whose length does not match `scheme`.
pub fn get_address_public_key(
    key_index: u32,
    scheme: CryptographicScheme,
) -> SysResult<PublicKey> {
    let mut buffer: MUBuf<PUBLIC_KEY_MAX_LEN> = MUBuf::new();
    let mut written: MaybeUninit<RuntimeMemoryLength> = MaybeUninit::uninit();
    sys::uninit_GetAddressPublicKey(
        key_index,
        buffer.spare_capacity_mut(),
        &mut written,
    )?;

    // As above: a report longer than any key means a truncated copy.
    let written = unsafe { written.assume_init() } as usize;
    if written > buffer.capacity() {
        return Err(
            SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.into()
        );
    }
    // SAFETY: the host initialized the first `written` bytes.
    unsafe { buffer.set_len(written) };

    if scheme == CryptographicScheme::Ed25519 {
        let bytes: [u8; 32] = buffer.as_slice().try_into().map_err(|_| {
            SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput
        })?;
        return Ok(PublicKey::Ed25519 {
            ed25519: Ed25519PublicKey::from(bytes),
        });
    }
    Err(SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.into())
}

/// Like [`get_address_public_key`], but returns `None` (instead of an error)
/// when the slot is empty.
pub fn try_get_address_public_key(
    key_index: u32,
    scheme: CryptographicScheme,
) -> SysResult<Option<PublicKey>> {
    match get_address_public_key(key_index, scheme) {
        Ok(key) => Ok(Some(key)),
        Err(e) if e.is_storage_key_not_exist() => Ok(None),
        Err(e) => Err(e),
    }
}
