#!/bin/sh

if [ $# -ne 0 ]; then
  echo "Usage: $0 "
  exit 1
fi

set -e

# Get current branch name. (POSIX test: this runs under /bin/sh, which is dash
# on Ubuntu/CI where `[[` is unavailable.)
if [ -z "${GIT_BRANCH}" ]; then
  branch=$(git rev-parse --abbrev-ref HEAD)
else
  branch=${GIT_BRANCH}
fi

echo "Branch: $branch"

# Get current commit hash
commit=$(git rev-parse HEAD)

# Get current commit timestamp in UTC ISO 8601 format (YYYY-MM-DDTHH:MM:SS)
commit_unix_time=$(git show -s --format=%ct HEAD)
if date -u -r "$commit_unix_time" "+%Y-%m-%dT%H:%M:%S" >/dev/null 2>&1; then
  # macOS/BSD date
  commit_time=$(date -u -r "$commit_unix_time" "+%Y-%m-%dT%H:%M:%S")
else
  # GNU date (Linux)
  commit_time=$(date -u -d "@$commit_unix_time" "+%Y-%m-%dT%H:%M:%S")
fi

echo "Branch: $branch"
echo "Commit: $commit"
echo "Commit Timestamp: $commit_time"

# Check if metrics_history/ is a git repo; if not, clone it
if [ ! -d "metrics_history/.git" ]; then
  echo "Cloning groundhogchain/metrics_history.git into metrics_history/"
  # SSH remote so the later `git push` authenticates via the CI ssh-agent
  # (SDK_TO_METRICS_HISTORY key); an HTTPS remote has no write credentials.
  git clone git@github.com:groundhogchain/metrics_history.git metrics_history/ -q
else
  echo "metrics_history/ already exists and is a git repository."
fi

produce_metric() {
    local metric="${1:?}"
    shift

    local metric_file="metrics_history/metrics/$metric/$branch/${commit_time}_$commit"
    local metric_script="./scripts/metrics/$metric.sh"

    # create metric file (and parent dirs as necessary)
    ./metrics_history/scripts/make_metric_file.sh $metric $branch $commit $commit_time

    # produce metric output
    "$metric_script" "$@" > "$metric_file" || {
        printf "%s: metric script failed", "$metric_script" >&2;
        exit 1;
    }
}

# METRIC: sdk_codesizes_rv32
produce_metric sdk_codesizes_rv32 "target/riscv32im-unknown-none-elf/release"

# METRIC: sdk_codesizes_rv32_lowered
produce_metric sdk_codesizes_rv32_lowered "target/thruput"

# Push metrics_history/ to groundhogchain/metrics_history.git
cd metrics_history/

# Check if git committer name and email are set; if not, set them to metrics-script
committer_name=$(git config user.name || true)
committer_email=$(git config user.email || true)
if [ -z "$committer_name" ]; then
  committer_name="metrics-script"
fi
if [ -z "$committer_email" ]; then
  committer_email="metrics-script@groundhogchain.com"
fi
echo "Committer name: $committer_name"
echo "Committer email: $committer_email"

git add metrics/
GIT_COMMITTER_NAME="$committer_name" \
  GIT_COMMITTER_EMAIL="$committer_email" \
  git commit -m "Add metrics for sdk_rs $branch $commit" --author "$committer_name <$committer_email>"

# Retry on push rejection: another repo's metrics job may have pushed
# concurrently, so rebase onto the updated remote and try again.
while ! git push; do
  echo "Push failed, attempting pull --rebase and retry..."
  GIT_COMMITTER_NAME="$committer_name" \
    GIT_COMMITTER_EMAIL="$committer_email" \
    git pull --rebase
done
