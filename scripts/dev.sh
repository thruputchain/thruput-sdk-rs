set -u

expand_contract() {
    contract="$1"; shift

    (
        cd "$contract" &&
        cargo --config ../.cargo/contract.toml \
            expand \
            --features "sdk/std" \
            --color=always \
        #
    )
}

action="$1"; shift

case "$action" in
    expand)
        expand_contract "$1"
        ;;
    * )
        echo "Sorry, didn't understand action '$action'."
        return 1
        ;;
esac
