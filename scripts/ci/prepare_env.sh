echo "prepare the environment for building hogd"

set -ex

echo 'setting up cargo'
source ./scripts/ci/setup_cargo.sh

echo 'setting up cache'
ln -s "$USER_HOME/.cache" "$HOME/.cache"
