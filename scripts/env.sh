TARGET_DIR="target"
HOGD_SRC="hogd"
HOGD_SRC="$(realpath "${HOGD_SRC}")"
CC=gcc
CXX=g++
DENY_WARNINGS=1

# Other useful settings:
#
## run cargo without --locked
# CARGO_UNLOCK=1
#
## enable logging of syscalls in the test executor
# SDK_TEST_ENABLE_SYSCALL_LOGGING=1
#
## don't turn cargo/rust warnings into errors
# unset DENY_WARNINGS

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
