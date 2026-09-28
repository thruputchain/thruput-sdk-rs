# Install the pinned groundhog CLI. Sourced, not executed: when
# GROUNDHOG_CLI_ROOT is set it prepends that root's bin to PATH, which the
# caller then keeps.
#
# Installs the version scripts/groundhog-cli-version.sh pins and skips the
# install when that version already answers `groundhog --version`, so re-runs
# with a warm cargo root are free. CI sets GROUNDHOG_CLI_ROOT to a
# Namespace-cached directory so the compiled binary survives across runs and
# the install is a no-op until the pin moves. Unset (local runs): installs to
# the default cargo root.
#
# `cargo install` deliberately ignores the repo's .cargo/config.toml (install
# is global, not project-scoped), so the registry index must arrive via
# environment config. Keep in sync with .cargo/config.toml [registries]. Auth
# comes from CARGO_REGISTRIES_GROUNDHOG_TOKEN +
# CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS in the workflows.

install_groundhog_cli() {
    local sdk_dir
    sdk_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    . "${sdk_dir}/scripts/groundhog-cli-version.sh"

    export CARGO_REGISTRIES_GROUNDHOG_INDEX="sparse+https://crates.thruput.network/api/v1/crates/"

    local -a install_flags=()
    if [ -n "${GROUNDHOG_CLI_ROOT:-}" ]; then
        export PATH="${GROUNDHOG_CLI_ROOT}/bin:${PATH}"
        install_flags=(--root "${GROUNDHOG_CLI_ROOT}")
    fi

    if [ "$(groundhog --version 2>/dev/null | awk '{print $2}')" != "$GROUNDHOG_CLI_VERSION" ]; then
        cargo install groundhog-cli --version "=$GROUNDHOG_CLI_VERSION" \
            --registry groundhog --locked "${install_flags[@]}"
    fi
}

install_groundhog_cli
