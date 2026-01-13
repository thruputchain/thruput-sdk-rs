#!/bin/sh

if [ $# -ne 0 ]; then
  echo "Usage: $0 "
  exit 1
fi

set -e

# Get current branch name
if [[ -z ${GITHUB_BRANCH} ]]; then
  branch=$(git rev-parse --abbrev-ref HEAD)
else
  branch=${GITHUB_BRANCH}
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

# Check if the working directory is dirty (tracked files only), ignoring .cargo/
if [ -n "$(git status --porcelain --untracked-files=no | grep -vE '^\s*M\s*\.cargo/')" ]; then
  echo "Working directory is dirty (except .cargo); cannot replicably run metrics"
  exit 1
fi

echo "Branch: $branch"
echo "Commit: $commit"
echo "Commit Timestamp: $commit_time"

# Check if metrics_history/ is a git repo; if not, clone it
if [ ! -d "metrics_history/.git" ]; then
  echo "Cloning groundhogchain/metrics_history.git into metrics_history/"
  git clone https://github.com/groundhogchain/metrics_history.git metrics_history/ -q
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

# METRIC: sdk_codesizes
produce_metric sdk_codesizes "target/optimized"

# METRIC: sdk_perf_gas
produce_metric sdk_perf_gas

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
git push
