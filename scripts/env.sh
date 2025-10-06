TARGET_DIR="target"
HOGD_SRC="hogd"
HOGD_SRC="$(realpath "${HOGD_SRC}")"
CC=gcc
CXX=g++

SCRIPT_DIR="$(dirname "$(realpath "$0")")"

if [ -f "${SCRIPT_DIR}/conf.sh" ]; then
    source "${SCRIPT_DIR}/conf.sh"
fi
