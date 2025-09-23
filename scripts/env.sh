TARGET_DIR="target"
HOGD_SRC="hogd"
HOGD_SRC="$(realpath "${HOGD_SRC}")"
CC=gcc
CXX=g++

if [ -f "conf.sh" ]; then
    source "conf.sh"
fi
