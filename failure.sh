#!/bin/sh

# T[0210 22:07:22.936627 runtime.cc:83] Invoke result: success, gas consumed: 0
# wasm-29f80508eb4ff246: FFI: root_invoke(syscall_handler=0x7f7dacab76a0) -> (result={.panic=0}, gas_consumed=0, ffi_error_code=0)
# T[0210 22:07:22.937002 rpc_from_exec.cc:161] Trying to push new subtransaction for block 1 and transaction 0
# T[0210 22:07:22.937616 invoke_tx.cc:107] failed to lock balance item for address faa24ea86fdc7408312bd31714d32b3a5ecef93793205fad719acbb4e8e824f2: Balance{delta=-69990, plus=0, minus=-69990}
# T[0210 22:07:22.937665 rpc_from_exec.cc:188] Failed to acquire balance locks after batch of deltas: Conflict

TEST_FFI_TRACE=1 SDK_TEST_ENABLE_SYSCALL_LOGGING=1 cargo test -p exceptions --test wasm degenerate -- --show-output
