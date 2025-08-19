

use groundhog_xdr::genesis::GenesisContractAddresses;
use groundhog_xdr::types::ContractAddress;

pub const fn get_precompile_addr(addr: GenesisContractAddresses) -> ContractAddress {
    let mut bytes : [u8; 32] = [0; 32];
    let addr_bytes = (addr as i32).to_be_bytes();
    bytes[28] = addr_bytes[0];
    bytes[29] = addr_bytes[1];
    bytes[30] = addr_bytes[2];
    bytes[31] = addr_byte   s[3];
    ContractAddress::new(bytes)
}

pub const ADDRESS_PRECOMPILED_HOG_TOKEN : ContractAddress 
    = get_precompile_addr(GenesisContractAddresses::kAddressPrecompiledHogToken);
pub const ADDRESS_INITIAL_TOKEN_CLAIM : ContractAddress 
    = get_precompile_addr(GenesisContractAddresses::kAddressInitialTokenClaim);
