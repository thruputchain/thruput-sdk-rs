# shellcheck shell=bash

SCRIPT_DIR="$(dirname "$(realpath "$0")")"
source "${SCRIPT_DIR}/lib_util.sh"

# Variables here may be overriden in the environment or in `scripts/conf.sh`
# (which may overwrite the environment).
#
# For boolean flags in `scripts/`, variables that are unset, empty, or equal to
# words like "false", "no", or "0" are false; other variables are true.  See
# `lib_util.sh` for the complete definition.

### Set some defaults

# For cargo:
: ${TARGET_DIR:="target"}

# For native_exec:
: ${CC:="gcc"}
: ${CXX:="g++"}
: ${HOGD_SRC:="hogd"}
HOGD_SRC="$(realpath "${HOGD_SRC}")"

# Fail build on cargo warnings?
: ${DENY_WARNINGS:="yes"}

### Document default-unset variables; uncomment to enable

## Run cargo without --locked?
# CARGO_UNLOCK=yes

## For wasm contracts, use the log() syscall to report panic messages
# PANIC_LOGGING=yes

## For native testing, print diagnostics for each contract call
# SDK_CALL_LOGGING=1

## Enable logging of syscalls in the test executor
## (This variable is interrogated at *runtime*.)
# SDK_TEST_ENABLE_SYSCALL_LOGGING=yes

## Enable tracking of contract calls for abort backtraces
# CALL_TRACKING=yes

## Enable tracing of the test-ffi interface
## (This variable is interrogated at *runtime*.)
# TEST_FFI_TRACE=1

## Use an already-built contract-wasm for bottle-tests instead of rebuilding it
## (This is set in ci.sh, which builds contracts before running tests.)
# BOTTLE_TEST_USE_BUILT_TARGET=yes

## Always do a cmake reconfiguration before building native_exec
# NATIVE_EXEC_ALWAYS_RECONFIGURE=yes

# Augment/override with any local settings
# (Note that `conf.sh` may override settings already in the environment,
# depending on how it is written.)
[ -f "${SCRIPT_DIR}/conf.sh" ] &&
    source "${SCRIPT_DIR}/conf.sh"

# In case these were set in `conf.sh`, export them for subprocesses

if bool_true "${SDK_TEST_ENABLE_SYSCALL_LOGGING:-}"; then
    export SDK_TEST_ENABLE_SYSCALL_LOGGING
fi

if bool_true "${NATIVE_EXEC_ALWAYS_RECONFIGURE:-}"; then
    export NATIVE_EXEC_ALWAYS_RECONFIGURE
fi

# Export other variables needed by subprocesses

if bool_true "${DENY_WARNINGS:-}"; then
    # error on warnings (affects all cargo warnings: rustc, clippy, etc.)
    export RUSTFLAGS="${RUSTFLAGS:-} -D warnings"
fi
