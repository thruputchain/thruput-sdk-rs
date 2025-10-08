TARGET_DIR="target"
HOGD_SRC="hogd"
HOGD_SRC="$(realpath "${HOGD_SRC}")"
CC=gcc
CXX=g++

# Settings that can go here or in conf.sh:
#
# CARGO_UNLOCK=1   # run cargo without --locked

SCRIPT_DIR="$(dirname "$(realpath "$0")")"

if [ -f "${SCRIPT_DIR}/conf.sh" ]; then
    source "${SCRIPT_DIR}/conf.sh"
fi
