#!/usr/bin/env bash
# Copied from hogd/.github/actions/private-sources; keep the two in sync.

set +x
set -euo pipefail
umask 077

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <token> <config-path>" >&2
  exit 2
fi
token="$1"
config_path="$2"
# Preserve the bootstrap rewrite and other job-local settings.
touch "$config_path"
chmod 600 "$config_path"

# Resolve and include the original globals before the caller changes the override.
# An existing bootstrap config already has its includes at the beginning.
if [[ ! -s "$config_path" ]]; then
  for original_config in "${XDG_CONFIG_HOME:-$HOME/.config}/git/config" "$HOME/.gitconfig"; do
    if [[ -f "$original_config" ]]; then
      git config --file "$config_path" --add include.path "$original_config"
    fi
  done
fi

# Full .git URLs prevent prefix collisions between protocol repositories.
# Explicit .git entries win over no-suffix HTTPS entries (longest match).
while read -r old new; do
  if [[ -z "$token" || "$token" == '<no value>' ]]; then
    target="git@github.com:thruputchain/${new}.git"
    git config --file "$config_path" --add "url.${target}.insteadOf" "https://github.com/thruputchain/${new}.git"
    git config --file "$config_path" --add "url.${target}.insteadOf" "https://github.com/thruputchain/${new}"
    continue
  fi
  target="https://x-access-token:${token}@github.com/thruputchain/${new}.git"
  for source in "groundhogchain/$old" "thruputchain/$new"; do
    git config --file "$config_path" --add "url.${target}.insteadOf" "git@github.com:${source}.git"
    git config --file "$config_path" --add "url.${target}.insteadOf" "ssh://git@github.com/${source}.git"
    git config --file "$config_path" --add "url.${target}.insteadOf" "https://github.com/${source}.git"
    git config --file "$config_path" --add "url.${target}.insteadOf" "https://github.com/${source}"
  done
done <<'REPOSITORIES'
hogd hogd
chuckd chuckd
hogx hogx
protocol thruput-protocol
protocol-go thruput-protocol-go
protocol-rs thruput-protocol-rs
lib-go lib-go
groundhog-go thruput-go
integration_test_contracts thruput-integration-test-contracts
scp scp
sdk_rs thruput-sdk-rs
REPOSITORIES
