# Pinned Thruput CLI version and optional source commit.
#
# Single source of truth: sourced by scripts/ci.sh (which installs exactly
# this version before dev.sh ci) and by scripts/dev.sh (which warns when the
# local CLI differs — CI is authoritative for metrics/artifacts). Bump this
# version when a compatible thruput-cli release is published. Until then,
# pin its source so SDK format changes can be tested before the CLI release.
THRUPUT_CLI_VERSION=0.27.0
# Recognize the renamed contract-spec package during dependency preflight.
THRUPUT_CLI_COMMIT=0aea0d8942a57038d902c7a6a23617be962fe25c
