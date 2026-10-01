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
    AddressAuthData, PublicKeyConfig, PublicKeyConfigOpt,
};
pub use groundhog_xdr::auth_claim::BitMap;
pub use groundhog_xdr::auth_claim::Weight;
use groundhog_xdr::syscall_error::SyscallErrorCode;
use groundhog_xdr::types::PublicKey;

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

/// Encoded size of the largest `AddressAuthData` (every slot occupied), so a
/// buffer this size holds any registration the host can report.
const ADDRESS_AUTH_DATA_MAX_LEN: usize =
    match <AddressAuthData as ConstMaxSize>::MAX_SIZE {
        Some(len) => len,
        None => panic!("AddressAuthData has a finite encoded size"),
    };

/// Reads the executing contract's own key registration: the `kMaxSigners`
/// weighted key slots, the required weight, and the height at which the row
/// was last written, as the current call frame sees them.
///
/// That is the committed row with this transaction's pending
/// [`add_address_public_key`], [`remove_address_public_key`] and
/// [`set_address_required_weight`] calls applied, the same way storage reads
/// see same-transaction writes; while such a write is pending,
/// `last_written_height` is the executing block height. An address with no
/// registration row reads as every slot empty with `required_weight` 0, not
/// as an error.
///
/// Admission checks committed state only: a key registered in this
/// transaction cannot authorize anything until the block carrying it lands.
pub fn get_address_auth_data() -> SysResult<AddressAuthData> {
    let mut buffer: MUBuf<ADDRESS_AUTH_DATA_MAX_LEN> = MUBuf::new();
    let mut written: MaybeUninit<RuntimeMemoryLength> = MaybeUninit::uninit();
    sys::uninit_GetAddressAuthData(buffer.spare_capacity_mut(), &mut written)?;

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
    AddressAuthData::read_xdr(&mut reader).map_err(|_| {
        SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.into()
    })
}
