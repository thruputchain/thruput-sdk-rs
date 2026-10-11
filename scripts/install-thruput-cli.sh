# Install the pinned Thruput CLI. Sourced, not executed: when
# THRUPUT_CLI_ROOT is set it prepends that root's bin to PATH, which the
# caller then keeps.
#
# Installs the version or source commit scripts/thruput-cli-version.sh pins.
# CI caches THRUPUT_CLI_ROOT; source installs record the commit as well as
# checking the version, so another build of the same version cannot satisfy
# the pin. The source checkout is also used by the scaffold smoke test.
#
# `cargo install` deliberately ignores the repo's .cargo/config.toml (install
# is global, not project-scoped), so the registry index must arrive via
# environment config. Keep in sync with .cargo/config.toml [registries]. Auth
# comes from CARGO_REGISTRIES_GROUNDHOG_TOKEN +
# CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS in the workflows.

install_thruput_cli() {
    local sdk_dir
    sdk_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    . "${sdk_dir}/scripts/thruput-cli-version.sh"

    export CARGO_REGISTRIES_GROUNDHOG_INDEX="sparse+https://crates.thruput.network/api/v1/crates/"

    if [ -n "${THRUPUT_CLI_COMMIT:-}" ]; then
        if [[ ! "$THRUPUT_CLI_COMMIT" =~ ^[0-9a-f]{40}$ ]]; then
            echo "error: THRUPUT_CLI_COMMIT must be a full lowercase commit SHA" >&2
            return 1
        fi
        local cli_root source_dir
        cli_root="${THRUPUT_CLI_ROOT:-${CARGO_HOME:-$HOME/.cargo}/thruput-cli}"
        mkdir -p "$cli_root"
        cli_root="$(cd "$cli_root" && pwd)"
        export PATH="${cli_root}/bin:${PATH}"
        # Keep this independent workspace outside the SDK checkout. Nesting
        # it there confuses workspace inheritance and SDK version scanning.
        source_dir="${THRUPUT_CLI_SOURCE_ROOT:-${CARGO_HOME:-$HOME/.cargo}/thruput-cli-source}/${THRUPUT_CLI_COMMIT}"
        if [ ! -d "$source_dir/.git" ]; then
            git init "$source_dir" || return
            git -C "$source_dir" remote add origin git@github.com:groundhogchain/groundhog-rpc.git || return
        fi
        if [ "$(git -C "$source_dir" rev-parse HEAD 2>/dev/null)" != "$THRUPUT_CLI_COMMIT" ]; then
            git -C "$source_dir" fetch --depth 1 origin "$THRUPUT_CLI_COMMIT" || return
            git -C "$source_dir" -c core.hooksPath=/dev/null checkout --detach "$THRUPUT_CLI_COMMIT" || return
        fi
        git -C "$source_dir" diff --quiet HEAD || return
        export THRUPUT_CLI_SOURCE_DIR="$source_dir"
        if [ "$(cat "$cli_root/.source-commit" 2>/dev/null)" != "$THRUPUT_CLI_COMMIT" ] ||
           [ "$("$cli_root/bin/thruput" --version 2>/dev/null | awk '{print $2}')" != "$THRUPUT_CLI_VERSION" ]; then
            # The CLI's stable toolchain is distinct from the SDK's nightly.
            local toolchain build_dir
            toolchain=$(python3 - "$source_dir/rust-toolchain.toml" <<'PY'
import sys
import tomllib
with open(sys.argv[1], 'rb') as config:
    print(tomllib.load(config)['toolchain']['channel'])
PY
            ) || return
            build_dir=$(mktemp -d "${RUNNER_TEMP:-/tmp}/thruput-cli-build.XXXXXX") || return
            (
                trap 'rm -rf "$build_dir"' EXIT
                cd "$source_dir" || exit
                GROUNDHOG_SOURCE_COMMIT="$THRUPUT_CLI_COMMIT" \
                    CARGO_HTTP_MULTIPLEXING=false \
                    cargo "+$toolchain" install --path crates/cli --locked --force \
                    --root "$cli_root" --target-dir "$build_dir"
            ) || return
            if [ "$("$cli_root/bin/thruput" --version | awk '{print $2}')" != "$THRUPUT_CLI_VERSION" ]; then
                echo "error: source CLI version does not match THRUPUT_CLI_VERSION" >&2
                return 1
            fi
            printf '%s\n' "$THRUPUT_CLI_COMMIT" > "$cli_root/.source-commit"
        fi
        return
    fi

    local -a install_flags=()
    if [ -n "${THRUPUT_CLI_ROOT:-}" ]; then
        export PATH="${THRUPUT_CLI_ROOT}/bin:${PATH}"
        install_flags=(--root "${THRUPUT_CLI_ROOT}")
    fi

    if [ "$(thruput --version 2>/dev/null | awk '{print $2}')" != "$THRUPUT_CLI_VERSION" ]; then
        cargo install thruput-cli --version "=$THRUPUT_CLI_VERSION" \
            --registry groundhog --locked "${install_flags[@]}"
    fi
}

install_thruput_cli
