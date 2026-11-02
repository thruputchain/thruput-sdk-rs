#!/usr/bin/env bash
# Write a job-local Git config that fetches hogd with an App token and every
# other GitHub repository anonymously over HTTPS.

set +x
set -euo pipefail
umask 077

if [[ $# -ne 2 || -z "$1" ]]; then
  echo "usage: $0 <token> <config-path>" >&2
  exit 2
fi
token="$1"
config_path="$2"
touch "$config_path"
chmod 600 "$config_path"

# Resolve and include the original globals before the caller changes the override.
for original_config in "${XDG_CONFIG_HOME:-$HOME/.config}/git/config" "$HOME/.gitconfig"; do
  if [[ -f "$original_config" ]]; then
    git config --file "$config_path" --add include.path "$original_config"
  fi
done

# hogd's submodules are public but use SSH URLs, which need a key even for
# public repositories. Fetch them over anonymous HTTPS instead.
git config --file "$config_path" --add "url.https://github.com/.insteadOf" "git@github.com:"
git config --file "$config_path" --add "url.https://github.com/.insteadOf" "ssh://git@github.com/"

# Only hogd is private. Git applies the longest matching prefix, so these
# entries take precedence over the anonymous rewrites above.
target="https://x-access-token:${token}@github.com/thruputchain/hogd.git"
for source in \
  "git@github.com:thruputchain/hogd.git" \
  "ssh://git@github.com/thruputchain/hogd.git" \
  "https://github.com/thruputchain/hogd.git" \
  "https://github.com/thruputchain/hogd"; do
  git config --file "$config_path" --add "url.${target}.insteadOf" "$source"
done
