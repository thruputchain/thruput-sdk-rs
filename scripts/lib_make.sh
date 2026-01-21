# shellcheck shell=bash

set -u

# shellcheck source="scripts/lib_make_examples.sh"
source "$SDK_SCRIPT_DIR"/lib_make_examples.sh

cargo() {
  local locked="--locked"

  if bool_true "${CARGO_UNLOCK:-}"; then
    locked=""
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
            cargo doc --features=std,storage,abort/debug "$@"
            ;;
        test)
            cargo_native test --package=sdk --features=mock,storage "$@"
            ;;
        nextest)
            ## There is at least one example contract ("caller") that requires
            ## pre-built wasms, so first do:
            make examples build

            cargo_native nextest run \
                --workspace \
                --features=sdk/mock,sdk/storage \
                --exclude txs_roundtrip \
                --exclude cxs_roundtrip \
                "$@"
            ;;
        test-spec)
            cargo run -p groundhog-erc20-if -F spec --bin show_spec >/dev/null &&
                cargo run -p fibonacci -F spec --bin generate_spec_fibonacci -- /dev/null &&
                cargo run -p tuple_return -F spec --bin generate_spec_tuple_return -- /dev/null
            ;;
        rg)
            rg -i --ignore-file .local_src_ignore "$@"
            ;;
        todo | TODO)
            rg -i --ignore-file .local_src_ignore TODO
            ;;
        xxx | XXX)
            rg -i \
                --ignore-file .local_src_ignore \
                --ignore-file .xxx_ignore \
                XXX
            ;;
        xxx-check)
            local rg

            # capture exit code without triggering errexit
            make xxx && rg=$? || rg=$?

            if [ $rg -ne 1 ]; then
                # Matches found, or error
                printf "\n>> XXX annotations found!\n"
                return 1
            else
                return 0
            fi
            ;;
        lint)
            cargo clippy --features=std "$@"
            ;;
        lint-mock)
            cargo_native clippy --features=mock "$@"
            ;;
        lint-check)
            make lint "$@" >& /dev/null || {
                printf "\n>> Trapped lint!\n"
                return 1;
            }
            ;;
        fmt)
            cargo fmt "$@"
            ;;
        fmt-check)
            make fmt --check --quiet "$@" || {
                printf "\n>> Formatting required!\n"
                return 1;
            }
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
