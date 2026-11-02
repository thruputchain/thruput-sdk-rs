# Pinned Thruput CLI version and optional source commit.
#
# Single source of truth: sourced by scripts/install-thruput-cli.sh (which
# `cargo gnaw setup` runs) and read by gnaw's `cli` task (which warns when the
# local CLI differs — CI is authoritative for metrics/artifacts). Bump this
# version when a compatible release of the CLI (`thruput` on crates.io) is
# published. Until then, pin its source so SDK format changes can be tested
# before the CLI release.
THRUPUT_CLI_VERSION=0.42.0
