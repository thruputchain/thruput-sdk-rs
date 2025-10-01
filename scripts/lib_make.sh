# shellcheck shell=bash

set -u

cargo() {
  local locked=""

  if [ -n "${CARGO_UNLOCK:-}" ]; then
    locked="--locked"
  fi

  env cargo $locked "$@"
}

# use `cargo_native` for tests using `native_exec`
cargo_native() {
    if [ ! -d "${HOGD_SRC}" ]; then
        echo "Couldn't find hogd sources at ${HOGD_SRC}"
        return 1
    fi

    # pull in build config
    SCRIPT_DIR="${HOGD_SRC}/scripts"
    # shellcheck source="hogd/scripts/env.sh"
    source "${SCRIPT_DIR}/env.sh"

    CC="${CC:-}" CXX="${CXX:-}" HOGD_SRC="${HOGD_SRC}" \
        cargo "$@"
}

# shellcheck source="scripts/lib_make_examples.sh"
source "$SDK_SCRIPT_DIR"/lib_make_examples.sh

make() {
    local action="$1"
    shift

    case "$action" in
        examples)
            # e.g. "examples build erc20"
            make_contracts "examples" "$@"
            ;;
        integration_test_contracts)
            # e.g. "integration_test_contracts build erc20"
            make_contracts "integration_test_contracts" "$@"
            ;;
        doc)
            cargo doc --features=std "$@"
            ;;
        test)
            cargo_native test --features=mock,storage "$@"
            ;;
        test-spec)
            cargo run -p groundhog-erc20-if -F spec --bin show_spec >/dev/null &&
                cargo run -p fibonacci -F spec --bin generate_spec -- /dev/null
            ;;
#        check)
#            cargo check --features=mock "$@"
#            ;;
        lint)
            cargo clippy --no-deps --features=std "$@"
            ;;
        lint_mock)
            cargo_native clippy --no-deps --features=mock "$@"
            ;;
        lint_macros)
            cargo clippy --no-deps -p macros "$@"
            ;;
        fmt)
            cargo fmt --package \
                macros \
                groundhog_xdr \
                protocol_macros \
                native_exec \
                "$(ls_contracts examples)" \
                "$@"
            ;;
        tree)
            # Show the dependency graph
            cargo tree \
                --target wasm32-unknown-unknown \
                --edges normal,build \
                "$@"
            ;;
        *)
            echo "Sorry, didn't understand action '$action'."
            return 1
            ;;
    esac
}
