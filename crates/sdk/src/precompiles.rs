use crate::ContractAddress;
use crate::genesis::GenesisContractAddresses;

pub const fn get_precompile_addr(
    addr: GenesisContractAddresses,
) -> ContractAddress {
    let mut bytes: [u8; 32] = [0; 32];
    let addr_bytes = (addr.0).to_be_bytes();
    bytes[28] = addr_bytes[0];
    bytes[29] = addr_bytes[1];
    bytes[30] = addr_bytes[2];
    bytes[31] = addr_bytes[3];
    ContractAddress::new(bytes)
}

pub const ADDRESS_NATIVE_TOKEN: ContractAddress =
    get_precompile_addr(GenesisContractAddresses::kAddressNativeToken);
pub const ADDRESS_GENESIS_ROOT: ContractAddress =
    get_precompile_addr(GenesisContractAddresses::kAddressGenesisRoot);

// Compatibility name for the native-token address used before Genesis V2.
pub const ADDRESS_PRECOMPILED_HOG_TOKEN: ContractAddress =
    ADDRESS_NATIVE_TOKEN;
// Compatibility name for the genesis-root address used before Genesis V2.
pub const ADDRESS_INITIAL_TOKEN_CLAIM: ContractAddress = ADDRESS_GENESIS_ROOT;
