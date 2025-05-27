
# shellcheck shell=bash

# A library function for use by `make.sh` in the
# parent directory as well as the one in this
# directory.
#
# use `cargo_native` for tests using `native_exec`

cargo="cargo +nightly"

function cargo_native {
    if [ -z "${HOGD_SRC}" ]; then
        HOGD_SRC="hogd"
    fi
    HOGD_SRC=$(realpath "${HOGD_SRC}")

    if [ ! -d "${HOGD_SRC}" ]; then
        echo "Couldn't find hogd sources at " "${HOGD_SRC}"
        exit 1
    fi

    # pull in build config
    SCRIPT_DIR="${HOGD_SRC}/scripts"
    # shellcheck source="../hogd/scripts/env.sh"
    source "${SCRIPT_DIR}/env.sh"

    CC="${CC}" CXX="${CXX}" HOGD_SRC="${HOGD_SRC}" \
        $cargo "$@"
}
