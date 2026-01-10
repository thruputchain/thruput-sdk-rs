set -x
echo 'setting up cargo'

ln -s "$USER_HOME/.cargo" "$HOME/.cargo"
ln -s "$USER_HOME/.rustup" "$HOME/.rustup"
source "$HOME/.cargo/env"

rustup toolchain list
