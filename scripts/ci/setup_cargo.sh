if [ -z "${CI:-}" ]; then
  echo "setup_cargo.sh wipes \$HOME/.cargo and is only for CI runners; set CI=1 to run it deliberately." >&2
  exit 1
fi

set -ex
echo 'setting up cargo'

# Get rid of the .cargo that's checked in to the repo;
# Right now, that just sets build flags when building on macos.
rm -r ${HOME}/.cargo

ln -s "$USER_HOME/.cargo" "$HOME/.cargo"
ln -s "$USER_HOME/.rustup" "$HOME/.rustup"
source "$HOME/.cargo/env"

rustup toolchain list
