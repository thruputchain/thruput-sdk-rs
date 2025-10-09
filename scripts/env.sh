TARGET_DIR="target"
HOGD_SRC="hogd"
HOGD_SRC="$(realpath "${HOGD_SRC}")"
CC=gcc
CXX=g++

# Other useful settings:
#
## run cargo without --locked
# CARGO_UNLOCK=1
#
## enable logging of syscalls in the test executor
# SDK_TEST_ENABLE_SYSCALL_LOGGING=1

# Include any local settings
SCRIPT_DIR="$(dirname "$(realpath "$0")")"
if [ -f "${SCRIPT_DIR}/conf.sh" ]; then
    source "${SCRIPT_DIR}/conf.sh"
fi
