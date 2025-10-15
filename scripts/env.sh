TARGET_DIR="target"
HOGD_SRC="hogd"
HOGD_SRC="$(realpath "${HOGD_SRC}")"
CC=gcc
CXX=g++
DENY_WARNINGS=1

# Other useful settings:

## Run cargo without --locked
# CARGO_UNLOCK=1

## Enable logging of syscalls in the test executor
# SDK_TEST_ENABLE_SYSCALL_LOGGING=1

## Don't turn cargo/rust warnings into errors
# unset DENY_WARNINGS

## Where available, run a contract's bottle-test instead of its package tests
# PREFER_BOTTLE_TEST=1

## Use an already-built contract wasm for bottle-tests instead of rebuilding it
# BOTTLE_TEST_USE_BUILT_TARGET=1

# Augment/override with any local settings
SCRIPT_DIR="$(dirname "$(realpath "$0")")"
if [ -f "${SCRIPT_DIR}/conf.sh" ]; then
    source "${SCRIPT_DIR}/conf.sh"
fi

# Export variables to be queried by subprocesses

if [ -n "$SDK_TEST_ENABLE_SYSCALL_LOGGING" ]; then
    export SDK_TEST_ENABLE_SYSCALL_LOGGING="${SDK_TEST_ENABLE_SYSCALL_LOGGING:?}"
fi

if [ -n "$DENY_WARNINGS" ]; then
    # error on warnings (affects all cargo warnings: rustc, clippy, etc.)
    export RUSTFLAGS="-D warnings"
fi
