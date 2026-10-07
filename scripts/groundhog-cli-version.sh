# Pinned groundhog CLI version and optional source commit.
#
# Single source of truth: sourced by scripts/ci.sh (which installs exactly
# this version before dev.sh ci) and by scripts/dev.sh (which warns when the
# local CLI differs — CI is authoritative for metrics/artifacts). Bump this
# version when a compatible groundhog-cli release is published. Until then,
# pin its source so SDK format changes can be tested before the CLI release.
GROUNDHOG_CLI_VERSION=0.25.0
GROUNDHOG_CLI_COMMIT=ef8769b4cd5f310c97a813c322d54d4d98c125bb
