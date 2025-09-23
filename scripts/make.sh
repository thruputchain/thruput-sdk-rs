#!/bin/bash

SDK_SCRIPT_DIR="$(dirname "$(realpath "$0")")"

# shellcheck source="scripts/env.sh"
source "${SDK_SCRIPT_DIR}/env.sh"

# shellcheck source="scripts/lib_make.sh"
source "${SDK_SCRIPT_DIR}/lib_make.sh"

make "$@"
