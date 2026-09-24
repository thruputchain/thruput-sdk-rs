# Pinned groundhog CLI version.
#
# Single source of truth: sourced by scripts/ci.sh (which installs exactly
# this version before dev.sh ci) and by scripts/dev.sh (which warns when the
# local CLI differs — CI is authoritative for metrics/artifacts). Bump this
# one line when a new groundhog-cli release is published.
GROUNDHOG_CLI_VERSION=0.19.0-pre.1
