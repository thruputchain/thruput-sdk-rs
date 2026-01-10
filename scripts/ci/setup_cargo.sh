set -ex
echo 'setting up cargo'

# Get rid of the .cargo that's checked in to the repo;
# Right now, that just sets build flags when building on macos.
rm -r ${HOME}/.cargo

ln -s "$USER_HOME/.cargo" "$HOME/.cargo"
ln -s "$USER_HOME/.rustup" "$HOME/.rustup"
source "$HOME/.cargo/env"

rustup toolchain list
