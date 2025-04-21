// -*-Rust-*-

// *** Generated Code  ***
// *** Do not check in ***

#![allow(non_snake_case)]

// These are for various, currently-unnecessary casts to/from u64 that
// are good to keep around in case types change
#![cfg_attr(not(feature="mock"), allow(clippy::unnecessary_cast))]
#![cfg_attr(feature="mock", allow(clippy::useless_conversion))]

use crate::types::*;
use crate::syscall::{SyscallNo, SysErr};
#[cfg(not(feature="mock"))]
use crate::syscall::{syscall_wrapper, to_offset, to_offset_mut};
#[cfg(not(feature="mock"))]
use crate::custom_arg::*;

use core::mem::MaybeUninit;
use core::hint::unreachable_unchecked;

#[cfg(feature="mock")]
use alloc::vec;
#[cfg(feature="mock")]
use ::groundhog_xdr::{ReadXdr, WriteXdr, ToXdr, FromXdr};
#[cfg(feature="mock")]
use ::native_exec::invoke_error::unrecoverable_error;
#[cfg(feature="mock")]
use std::panic::panic_any;

type Result<T> = core::result::Result<T, SysErr>;

#[cfg(not(feature="mock"))]
pub fn uninit_SHA256(buffer : &[u8], hash : &mut MaybeUninit<SHA256Hash>) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kSHA256,
            buffer.as_ptr() as u64,
            buffer.len() as u64,
            hash.as_mut_ptr() as u64,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_SHA256(buffer : &[u8], hash : &mut MaybeUninit<SHA256Hash>) -> Result<()> {
    let input = ::groundhog_xdr::SHA256_input {
        buffer : buffer.try_into().map_err(|_| panic_any(unrecoverable_error())).unwrap(),
    };

    // allocate space for syscall to write results
    let mut output = ::groundhog_xdr::SHA256_output { 
        hash : SHA256Hash::default(),
    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kSHA256,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }


    output =
        ::groundhog_xdr::SHA256_output::from_xdr(&out_buffer, ::groundhog_xdr::Limits::none())
            .map_err(|_| panic_any(unrecoverable_error()))
            .unwrap();

    unsafe {
        std::ptr::copy_nonoverlapping(output.hash.as_ptr(), hash.as_mut_ptr() as *mut u8, output.hash.len());
    }

    Ok(())

}

pub fn raw_SHA256(buffer : &[u8], hash : &mut SHA256Hash) -> Result<()> {
    uninit_SHA256(buffer, unsafe {core::mem::transmute::<&mut SHA256Hash, &mut MaybeUninit<SHA256Hash>>(hash)})
}

#[cfg(not(feature="mock"))]
pub fn uninit_GetMsgSender(sender : &mut MaybeUninit<ContractAddress>) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kGetMsgSender,
            sender.as_mut_ptr() as u64,
            0,
            0,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_GetMsgSender(sender : &mut MaybeUninit<ContractAddress>) -> Result<()> {
    let input = ::groundhog_xdr::GetMsgSender_input {

    };

    // allocate space for syscall to write results
    let mut output = ::groundhog_xdr::GetMsgSender_output { 
        sender : ContractAddress::default(),
    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kGetMsgSender,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }


    output =
        ::groundhog_xdr::GetMsgSender_output::from_xdr(&out_buffer, ::groundhog_xdr::Limits::none())
            .map_err(|_| panic_any(unrecoverable_error()))
            .unwrap();

    unsafe {
        std::ptr::copy_nonoverlapping(output.sender.as_ptr(), sender.as_mut_ptr() as *mut u8, output.sender.len());
    }

    Ok(())

}

pub fn raw_GetMsgSender(sender : &mut ContractAddress) -> Result<()> {
    uninit_GetMsgSender(unsafe {core::mem::transmute::<&mut ContractAddress, &mut MaybeUninit<ContractAddress>>(sender)})
}

#[cfg(not(feature="mock"))]
pub fn uninit_GetCalldataLen(len : &mut RuntimeMemoryLength) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kGetCalldataLen,
            to_offset_mut(len) as u64,
            0,
            0,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_GetCalldataLen(len : &mut RuntimeMemoryLength) -> Result<()> {
    let input = ::groundhog_xdr::GetCalldataLen_input {

    };

    // allocate space for syscall to write results
    let mut output = ::groundhog_xdr::GetCalldataLen_output { 
        len : 0,
    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kGetCalldataLen,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }


    output =
        ::groundhog_xdr::GetCalldataLen_output::from_xdr(&out_buffer, ::groundhog_xdr::Limits::none())
            .map_err(|_| panic_any(unrecoverable_error()))
            .unwrap();

    *len = output.len;

    Ok(())

}

pub fn raw_GetCalldataLen(len : &mut RuntimeMemoryLength) -> Result<()> {
    uninit_GetCalldataLen(len)
}

#[cfg(not(feature="mock"))]
pub fn uninit_GetCalldata(buffer : &mut [MaybeUninit<u8>], written_len : &mut RuntimeMemoryLength) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kGetCalldata,
            buffer.as_mut_ptr() as u64,
            buffer.len() as u64,
            to_offset_mut(written_len) as u64,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_GetCalldata(buffer : &mut [MaybeUninit<u8>], written_len : &mut RuntimeMemoryLength) -> Result<()> {
    let input = ::groundhog_xdr::GetCalldata_input {

    };

    // allocate space for syscall to write results
    let mut output = ::groundhog_xdr::GetCalldata_output { 
        buffer :  vec![0; buffer.len()].try_into().unwrap(),
        written_len : 0,
    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kGetCalldata,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }


    output =
        ::groundhog_xdr::GetCalldata_output::from_xdr(&out_buffer, ::groundhog_xdr::Limits::none())
            .map_err(|_| panic_any(unrecoverable_error()))
            .unwrap();

    unsafe {
        std::ptr::copy_nonoverlapping(output.buffer.as_ptr(), buffer.as_mut_ptr() as *mut u8, output.buffer.len());
    }
    *written_len = output.written_len;

    Ok(())

}

pub fn raw_GetCalldata(buffer : &mut [u8], written_len : &mut RuntimeMemoryLength) -> Result<()> {
    uninit_GetCalldata(unsafe {core::mem::transmute::<&mut [u8], &mut [MaybeUninit<u8>]>(buffer)}, written_len)
}

#[cfg(not(feature="mock"))]
pub fn uninit_Log(buffer : &[u8]) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kLog,
            buffer.as_ptr() as u64,
            buffer.len() as u64,
            0,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_Log(buffer : &[u8]) -> Result<()> {
    let input = ::groundhog_xdr::Log_input {
        buffer : buffer.try_into().map_err(|_| panic_any(unrecoverable_error())).unwrap(),
    };

    // allocate space for syscall to write results
    let output = ::groundhog_xdr::Log_output { 

    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kLog,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }





    Ok(())

}

pub fn raw_Log(buffer : &[u8]) -> Result<()> {
    uninit_Log(buffer)
}

#[cfg(not(feature="mock"))]
pub fn uninit_Invoke(address : &ContractAddress, method : MethodID, gas_limit : GasAmount, calldata : &[u8], flags : InvocationFlagSet) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kInvoke,
            address.as_ptr() as u64,
            method as u64,
            gas_limit as u64,
            calldata.as_ptr() as u64,
            calldata.len() as u64,
            flags as u64);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_Invoke(address : &ContractAddress, method : MethodID, gas_limit : GasAmount, calldata : &[u8], flags : InvocationFlagSet) -> Result<()> {
    let input = ::groundhog_xdr::Invoke_input {
        address : *address,
        method : method.try_into().map_err(|_| panic_any(unrecoverable_error())).unwrap(),
        gas_limit : gas_limit.try_into().map_err(|_| panic_any(unrecoverable_error())).unwrap(),
        calldata : calldata.try_into().map_err(|_| panic_any(unrecoverable_error())).unwrap(),
        flags : flags.try_into().map_err(|_| panic_any(unrecoverable_error())).unwrap(),
    };

    // allocate space for syscall to write results
    let output = ::groundhog_xdr::Invoke_output { 

    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kInvoke,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }





    Ok(())

}

pub fn raw_Invoke(address : &ContractAddress, method : MethodID, gas_limit : GasAmount, calldata : &[u8], flags : InvocationFlagSet) -> Result<()> {
    uninit_Invoke(address, method, gas_limit, calldata, flags)
}

#[cfg(not(feature="mock"))]
pub fn uninit_Return(buffer : &[u8]) -> ! {
    
    // syscall will initialize outputs, if error is kNone
    
        syscall_wrapper(SyscallNo::kReturn,
            buffer.as_ptr() as u64,
            buffer.len() as u64,
            0,
            0,
            0,
            0);

    unsafe {unreachable_unchecked()}
}

#[cfg(feature="mock")]
pub fn uninit_Return(buffer : &[u8]) -> ! {
    let input = ::groundhog_xdr::Return_input {
        buffer : buffer.try_into().map_err(|_| panic_any(unrecoverable_error())).unwrap(),
    };

    let mut out_buffer : [u8; 0] = [0; 0];

    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    ::native_exec::perform_mocked_syscall(
        SyscallNo::kReturn,
        &input_slice,
        &mut out_buffer,
    );
    unsafe {unreachable_unchecked()}
}

pub fn raw_Return(buffer : &[u8]) -> ! {
    uninit_Return(buffer)
}

#[cfg(not(feature="mock"))]
pub fn uninit_GetReturnedBuffer(buffer : &mut [MaybeUninit<u8>], written_len : &mut RuntimeMemoryLength) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kGetReturnedBuffer,
            buffer.as_mut_ptr() as u64,
            buffer.len() as u64,
            to_offset_mut(written_len) as u64,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_GetReturnedBuffer(buffer : &mut [MaybeUninit<u8>], written_len : &mut RuntimeMemoryLength) -> Result<()> {
    let input = ::groundhog_xdr::GetReturnedBuffer_input {

    };

    // allocate space for syscall to write results
    let mut output = ::groundhog_xdr::GetReturnedBuffer_output { 
        buffer :  vec![0; buffer.len()].try_into().unwrap(),
        written_len : 0,
    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kGetReturnedBuffer,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }


    output =
        ::groundhog_xdr::GetReturnedBuffer_output::from_xdr(&out_buffer, ::groundhog_xdr::Limits::none())
            .map_err(|_| panic_any(unrecoverable_error()))
            .unwrap();

    unsafe {
        std::ptr::copy_nonoverlapping(output.buffer.as_ptr(), buffer.as_mut_ptr() as *mut u8, output.buffer.len());
    }
    *written_len = output.written_len;

    Ok(())

}

pub fn raw_GetReturnedBuffer(buffer : &mut [u8], written_len : &mut RuntimeMemoryLength) -> Result<()> {
    uninit_GetReturnedBuffer(unsafe {core::mem::transmute::<&mut [u8], &mut [MaybeUninit<u8>]>(buffer)}, written_len)
}

#[cfg(not(feature="mock"))]
pub fn uninit_GetReturnedBufferLen(buffer_len : &mut RuntimeMemoryLength) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kGetReturnedBufferLen,
            to_offset_mut(buffer_len) as u64,
            0,
            0,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_GetReturnedBufferLen(buffer_len : &mut RuntimeMemoryLength) -> Result<()> {
    let input = ::groundhog_xdr::GetReturnedBufferLen_input {

    };

    // allocate space for syscall to write results
    let mut output = ::groundhog_xdr::GetReturnedBufferLen_output { 
        buffer_len : 0,
    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kGetReturnedBufferLen,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }


    output =
        ::groundhog_xdr::GetReturnedBufferLen_output::from_xdr(&out_buffer, ::groundhog_xdr::Limits::none())
            .map_err(|_| panic_any(unrecoverable_error()))
            .unwrap();

    *buffer_len = output.buffer_len;

    Ok(())

}

pub fn raw_GetReturnedBufferLen(buffer_len : &mut RuntimeMemoryLength) -> Result<()> {
    uninit_GetReturnedBufferLen(buffer_len)
}

#[cfg(not(feature="mock"))]
pub fn uninit_Exit(code : i32) -> ! {
    
    // syscall will initialize outputs, if error is kNone
    
        syscall_wrapper(SyscallNo::kExit,
            code as u64,
            0,
            0,
            0,
            0,
            0);

    unsafe {unreachable_unchecked()}
}

#[cfg(feature="mock")]
pub fn uninit_Exit(code : i32) -> ! {
    let input = ::groundhog_xdr::Exit_input {
        code : code.try_into().map_err(|_| panic_any(unrecoverable_error())).unwrap(),
    };

    let mut out_buffer : [u8; 0] = [0; 0];

    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    ::native_exec::perform_mocked_syscall(
        SyscallNo::kExit,
        &input_slice,
        &mut out_buffer,
    );
    unsafe {unreachable_unchecked()}
}

pub fn raw_Exit(code : i32) -> ! {
    uninit_Exit(code)
}

#[cfg(not(feature="mock"))]
pub fn uninit_GetDeployAddress(caller_address : Option<&ContractAddress>, nonce : &SHA256Hash, contract_hash : &SHA256Hash, to_be_deployed : &mut MaybeUninit<ContractAddress>) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kGetDeployAddress,
            match caller_address { Some(x) => {x.as_ptr() as u64 }, None => u64::MAX },
            nonce.as_ptr() as u64,
            contract_hash.as_ptr() as u64,
            to_be_deployed.as_mut_ptr() as u64,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_GetDeployAddress(caller_address : Option<&ContractAddress>, nonce : &SHA256Hash, contract_hash : &SHA256Hash, to_be_deployed : &mut MaybeUninit<ContractAddress>) -> Result<()> {
    let input = ::groundhog_xdr::GetDeployAddress_input {
        caller_address : caller_address.copied(),
        nonce : *nonce,
        contract_hash : *contract_hash,
    };

    // allocate space for syscall to write results
    let mut output = ::groundhog_xdr::GetDeployAddress_output { 
        to_be_deployed : ContractAddress::default(),
    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kGetDeployAddress,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }


    output =
        ::groundhog_xdr::GetDeployAddress_output::from_xdr(&out_buffer, ::groundhog_xdr::Limits::none())
            .map_err(|_| panic_any(unrecoverable_error()))
            .unwrap();

    unsafe {
        std::ptr::copy_nonoverlapping(output.to_be_deployed.as_ptr(), to_be_deployed.as_mut_ptr() as *mut u8, output.to_be_deployed.len());
    }

    Ok(())

}

pub fn raw_GetDeployAddress(caller_address : Option<&ContractAddress>, nonce : &SHA256Hash, contract_hash : &SHA256Hash, to_be_deployed : &mut ContractAddress) -> Result<()> {
    uninit_GetDeployAddress(caller_address, nonce, contract_hash, unsafe {core::mem::transmute::<&mut ContractAddress, &mut MaybeUninit<ContractAddress>>(to_be_deployed)})
}

#[cfg(not(feature="mock"))]
pub fn uninit_DeployContract(nonce : &SHA256Hash, contract_hash : &SHA256Hash, deployed_address : &mut MaybeUninit<ContractAddress>) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kDeployContract,
            nonce.as_ptr() as u64,
            contract_hash.as_ptr() as u64,
            deployed_address.as_mut_ptr() as u64,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_DeployContract(nonce : &SHA256Hash, contract_hash : &SHA256Hash, deployed_address : &mut MaybeUninit<ContractAddress>) -> Result<()> {
    let input = ::groundhog_xdr::DeployContract_input {
        nonce : *nonce,
        contract_hash : *contract_hash,
    };

    // allocate space for syscall to write results
    let mut output = ::groundhog_xdr::DeployContract_output { 
        deployed_address : ContractAddress::default(),
    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kDeployContract,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }


    output =
        ::groundhog_xdr::DeployContract_output::from_xdr(&out_buffer, ::groundhog_xdr::Limits::none())
            .map_err(|_| panic_any(unrecoverable_error()))
            .unwrap();

    unsafe {
        std::ptr::copy_nonoverlapping(output.deployed_address.as_ptr(), deployed_address.as_mut_ptr() as *mut u8, output.deployed_address.len());
    }

    Ok(())

}

pub fn raw_DeployContract(nonce : &SHA256Hash, contract_hash : &SHA256Hash, deployed_address : &mut ContractAddress) -> Result<()> {
    uninit_DeployContract(nonce, contract_hash, unsafe {core::mem::transmute::<&mut ContractAddress, &mut MaybeUninit<ContractAddress>>(deployed_address)})
}

#[cfg(not(feature="mock"))]
pub fn uninit_GetSourceTransactionHash(source_tx_hash : &mut MaybeUninit<SHA256Hash>) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kGetSourceTransactionHash,
            source_tx_hash.as_mut_ptr() as u64,
            0,
            0,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_GetSourceTransactionHash(source_tx_hash : &mut MaybeUninit<SHA256Hash>) -> Result<()> {
    let input = ::groundhog_xdr::GetSourceTransactionHash_input {

    };

    // allocate space for syscall to write results
    let mut output = ::groundhog_xdr::GetSourceTransactionHash_output { 
        source_tx_hash : SHA256Hash::default(),
    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kGetSourceTransactionHash,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }


    output =
        ::groundhog_xdr::GetSourceTransactionHash_output::from_xdr(&out_buffer, ::groundhog_xdr::Limits::none())
            .map_err(|_| panic_any(unrecoverable_error()))
            .unwrap();

    unsafe {
        std::ptr::copy_nonoverlapping(output.source_tx_hash.as_ptr(), source_tx_hash.as_mut_ptr() as *mut u8, output.source_tx_hash.len());
    }

    Ok(())

}

pub fn raw_GetSourceTransactionHash(source_tx_hash : &mut SHA256Hash) -> Result<()> {
    uninit_GetSourceTransactionHash(unsafe {core::mem::transmute::<&mut SHA256Hash, &mut MaybeUninit<SHA256Hash>>(source_tx_hash)})
}

#[cfg(not(feature="mock"))]
pub fn uninit_BytestringGetLen(key : &StorageKey, length : &mut RuntimeMemoryLength) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kBytestringGetLen,
            key.as_ptr() as u64,
            to_offset_mut(length) as u64,
            0,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_BytestringGetLen(key : &StorageKey, length : &mut RuntimeMemoryLength) -> Result<()> {
    let input = ::groundhog_xdr::BytestringGetLen_input {
        key : *key,
    };

    // allocate space for syscall to write results
    let mut output = ::groundhog_xdr::BytestringGetLen_output { 
        length : 0,
    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kBytestringGetLen,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }


    output =
        ::groundhog_xdr::BytestringGetLen_output::from_xdr(&out_buffer, ::groundhog_xdr::Limits::none())
            .map_err(|_| panic_any(unrecoverable_error()))
            .unwrap();

    *length = output.length;

    Ok(())

}

pub fn raw_BytestringGetLen(key : &StorageKey, length : &mut RuntimeMemoryLength) -> Result<()> {
    uninit_BytestringGetLen(key, length)
}

#[cfg(not(feature="mock"))]
pub fn uninit_BytestringGet(key : &StorageKey, bytestring : &mut [MaybeUninit<u8>], written_len : &mut RuntimeMemoryLength) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kBytestringGet,
            key.as_ptr() as u64,
            bytestring.as_mut_ptr() as u64,
            bytestring.len() as u64,
            to_offset_mut(written_len) as u64,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_BytestringGet(key : &StorageKey, bytestring : &mut [MaybeUninit<u8>], written_len : &mut RuntimeMemoryLength) -> Result<()> {
    let input = ::groundhog_xdr::BytestringGet_input {
        key : *key,
    };

    // allocate space for syscall to write results
    let mut output = ::groundhog_xdr::BytestringGet_output { 
        bytestring :  vec![0; bytestring.len()].try_into().unwrap(),
        written_len : 0,
    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kBytestringGet,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }


    output =
        ::groundhog_xdr::BytestringGet_output::from_xdr(&out_buffer, ::groundhog_xdr::Limits::none())
            .map_err(|_| panic_any(unrecoverable_error()))
            .unwrap();

    unsafe {
        std::ptr::copy_nonoverlapping(output.bytestring.as_ptr(), bytestring.as_mut_ptr() as *mut u8, output.bytestring.len());
    }
    *written_len = output.written_len;

    Ok(())

}

pub fn raw_BytestringGet(key : &StorageKey, bytestring : &mut [u8], written_len : &mut RuntimeMemoryLength) -> Result<()> {
    uninit_BytestringGet(key, unsafe {core::mem::transmute::<&mut [u8], &mut [MaybeUninit<u8>]>(bytestring)}, written_len)
}

#[cfg(not(feature="mock"))]
pub fn uninit_BytestringSet(key : &StorageKey, bytestring : &[u8], flags : StorageDeltaFlags) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kBytestringSet,
            key.as_ptr() as u64,
            bytestring.as_ptr() as u64,
            bytestring.len() as u64,
            flags as u64,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_BytestringSet(key : &StorageKey, bytestring : &[u8], flags : StorageDeltaFlags) -> Result<()> {
    let input = ::groundhog_xdr::BytestringSet_input {
        key : *key,
        bytestring : bytestring.try_into().map_err(|_| panic_any(unrecoverable_error())).unwrap(),
        flags : flags.try_into().map_err(|_| panic_any(unrecoverable_error())).unwrap(),
    };

    // allocate space for syscall to write results
    let output = ::groundhog_xdr::BytestringSet_output { 

    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kBytestringSet,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }





    Ok(())

}

pub fn raw_BytestringSet(key : &StorageKey, bytestring : &[u8], flags : StorageDeltaFlags) -> Result<()> {
    uninit_BytestringSet(key, bytestring, flags)
}

#[cfg(not(feature="mock"))]
pub fn uninit_NonnegativeIntGet(key : &StorageKey, current_set : &mut MaybeUninit<i128>, current_add : &mut MaybeUninit<i128>) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kNonnegativeIntGet,
            key.as_ptr() as u64,
            current_set.as_mut_ptr() as u64,
            current_add.as_mut_ptr() as u64,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_NonnegativeIntGet(key : &StorageKey, current_set : &mut MaybeUninit<i128>, current_add : &mut MaybeUninit<i128>) -> Result<()> {
    let input = ::groundhog_xdr::NonnegativeIntGet_input {
        key : *key,
    };

    // allocate space for syscall to write results
    let mut output = ::groundhog_xdr::NonnegativeIntGet_output { 
        current_set : int128_xdr::default(),
        current_add : int128_xdr::default(),
    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kNonnegativeIntGet,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }


    output =
        ::groundhog_xdr::NonnegativeIntGet_output::from_xdr(&out_buffer, ::groundhog_xdr::Limits::none())
            .map_err(|_| panic_any(unrecoverable_error()))
            .unwrap();

    current_set.write(i128::from_xdr_struct(&output.current_set));
    current_add.write(i128::from_xdr_struct(&output.current_add));

    Ok(())

}

pub fn raw_NonnegativeIntGet(key : &StorageKey, current_set : &mut i128, current_add : &mut i128) -> Result<()> {
    uninit_NonnegativeIntGet(key, unsafe {core::mem::transmute::<&mut i128, &mut MaybeUninit<i128>>(current_set)}, unsafe {core::mem::transmute::<&mut i128, &mut MaybeUninit<i128>>(current_add)})
}

#[cfg(not(feature="mock"))]
pub fn uninit_NonnegativeIntSetAdd(key : &StorageKey, set_value : &i128, add_value : &i128) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kNonnegativeIntSetAdd,
            key.as_ptr() as u64,
            to_offset(set_value) as u64,
            to_offset(add_value) as u64,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_NonnegativeIntSetAdd(key : &StorageKey, set_value : &i128, add_value : &i128) -> Result<()> {
    let input = ::groundhog_xdr::NonnegativeIntSetAdd_input {
        key : *key,
        set_value : i128::to_xdr_struct(set_value),
        add_value : i128::to_xdr_struct(add_value),
    };

    // allocate space for syscall to write results
    let output = ::groundhog_xdr::NonnegativeIntSetAdd_output { 

    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kNonnegativeIntSetAdd,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }





    Ok(())

}

pub fn raw_NonnegativeIntSetAdd(key : &StorageKey, set_value : &i128, add_value : &i128) -> Result<()> {
    uninit_NonnegativeIntSetAdd(key, set_value, add_value)
}

#[cfg(not(feature="mock"))]
pub fn uninit_DeleteObject(key : &StorageKey) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kDeleteObject,
            key.as_ptr() as u64,
            0,
            0,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_DeleteObject(key : &StorageKey) -> Result<()> {
    let input = ::groundhog_xdr::DeleteObject_input {
        key : *key,
    };

    // allocate space for syscall to write results
    let output = ::groundhog_xdr::DeleteObject_output { 

    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kDeleteObject,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }





    Ok(())

}

pub fn raw_DeleteObject(key : &StorageKey) -> Result<()> {
    uninit_DeleteObject(key)
}

#[cfg(not(feature="mock"))]
pub fn uninit_VerifySignature(signature: &Signature, public_key: &PublicKey, message : &[u8]) -> Result<()> {
    let (signature_0, signature_1) = signature.to_u64s();
    let (public_key_0, public_key_1) = public_key.to_u64s();
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kVerifySignature,
            signature_0,
            signature_1,
            public_key_0,
            public_key_1,
            message.as_ptr() as u64,
            message.len() as u64);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_VerifySignature(signature: &Signature, public_key: &PublicKey, message : &[u8]) -> Result<()> {
    let input = ::groundhog_xdr::VerifySignature_input {
        signature : signature.clone(),
        public_key : public_key.clone(),
        message : message.try_into().map_err(|_| panic_any(unrecoverable_error())).unwrap(),
    };

    // allocate space for syscall to write results
    let output = ::groundhog_xdr::VerifySignature_output { 

    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kVerifySignature,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }





    Ok(())

}

pub fn raw_VerifySignature(signature: &Signature, public_key: &PublicKey, message : &[u8]) -> Result<()> {
    uninit_VerifySignature(signature, public_key, message)
}

#[cfg(not(feature="mock"))]
pub fn uninit_GetCurrentAddress(address : &mut MaybeUninit<ContractAddress>) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kGetCurrentAddress,
            address.as_mut_ptr() as u64,
            0,
            0,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_GetCurrentAddress(address : &mut MaybeUninit<ContractAddress>) -> Result<()> {
    let input = ::groundhog_xdr::GetCurrentAddress_input {

    };

    // allocate space for syscall to write results
    let mut output = ::groundhog_xdr::GetCurrentAddress_output { 
        address : ContractAddress::default(),
    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kGetCurrentAddress,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }


    output =
        ::groundhog_xdr::GetCurrentAddress_output::from_xdr(&out_buffer, ::groundhog_xdr::Limits::none())
            .map_err(|_| panic_any(unrecoverable_error()))
            .unwrap();

    unsafe {
        std::ptr::copy_nonoverlapping(output.address.as_ptr(), address.as_mut_ptr() as *mut u8, output.address.len());
    }

    Ok(())

}

pub fn raw_GetCurrentAddress(address : &mut ContractAddress) -> Result<()> {
    uninit_GetCurrentAddress(unsafe {core::mem::transmute::<&mut ContractAddress, &mut MaybeUninit<ContractAddress>>(address)})
}

#[cfg(not(feature="mock"))]
pub fn uninit_RetrocausalQueueInsert(key : &StorageKey, queue_entry : &[u8]) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kRetrocausalQueueInsert,
            key.as_ptr() as u64,
            queue_entry.as_ptr() as u64,
            queue_entry.len() as u64,
            0,
            0,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_RetrocausalQueueInsert(key : &StorageKey, queue_entry : &[u8]) -> Result<()> {
    let input = ::groundhog_xdr::RetrocausalQueueInsert_input {
        key : *key,
        queue_entry : queue_entry.try_into().map_err(|_| panic_any(unrecoverable_error())).unwrap(),
    };

    // allocate space for syscall to write results
    let output = ::groundhog_xdr::RetrocausalQueueInsert_output { 

    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kRetrocausalQueueInsert,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }





    Ok(())

}

pub fn raw_RetrocausalQueueInsert(key : &StorageKey, queue_entry : &[u8]) -> Result<()> {
    uninit_RetrocausalQueueInsert(key, queue_entry)
}

#[cfg(not(feature="mock"))]
pub fn uninit_RetrocausalQueueConsume(key : &StorageKey, peek : bool, queue_entry : &mut [MaybeUninit<u8>], written_len : &mut RuntimeMemoryLength) -> Result<()> {
    
    // syscall will initialize outputs, if error is kNone
    let err: SysErr = 
        syscall_wrapper(SyscallNo::kRetrocausalQueueConsume,
            key.as_ptr() as u64,
            peek as u64,
            queue_entry.as_mut_ptr() as u64,
            queue_entry.len() as u64,
            to_offset_mut(written_len) as u64,
            0);

    if !err.is_none() {
        return Err(err);
    }

    Ok(())
}

#[cfg(feature="mock")]
pub fn uninit_RetrocausalQueueConsume(key : &StorageKey, peek : bool, queue_entry : &mut [MaybeUninit<u8>], written_len : &mut RuntimeMemoryLength) -> Result<()> {
    let input = ::groundhog_xdr::RetrocausalQueueConsume_input {
        key : *key,
        peek : peek.try_into().map_err(|_| panic_any(unrecoverable_error())).unwrap(),
    };

    // allocate space for syscall to write results
    let mut output = ::groundhog_xdr::RetrocausalQueueConsume_output { 
        queue_entry :  vec![0; queue_entry.len()].try_into().unwrap(),
        written_len : 0,
    };
    let mut out_buffer = output
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();


    let input_slice = input
        .to_xdr(::groundhog_xdr::Limits::none())
        .map_err(|_| panic_any(unrecoverable_error()))
        .unwrap();

    let err : SysErr = SysErr(::native_exec::perform_mocked_syscall(
        SyscallNo::kRetrocausalQueueConsume,
        &input_slice,
        &mut out_buffer,
    ));

    if !err.is_none() {
        return Err(err);
    }


    output =
        ::groundhog_xdr::RetrocausalQueueConsume_output::from_xdr(&out_buffer, ::groundhog_xdr::Limits::none())
            .map_err(|_| panic_any(unrecoverable_error()))
            .unwrap();

    unsafe {
        std::ptr::copy_nonoverlapping(output.queue_entry.as_ptr(), queue_entry.as_mut_ptr() as *mut u8, output.queue_entry.len());
    }
    *written_len = output.written_len;

    Ok(())

}

pub fn raw_RetrocausalQueueConsume(key : &StorageKey, peek : bool, queue_entry : &mut [u8], written_len : &mut RuntimeMemoryLength) -> Result<()> {
    uninit_RetrocausalQueueConsume(key, peek, unsafe {core::mem::transmute::<&mut [u8], &mut [MaybeUninit<u8>]>(queue_entry)}, written_len)
}



