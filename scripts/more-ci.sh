set -eu

todo() {
    rg -i --ignore-file .local_src_ignore TODO
}

xxx() {
    rg -i \
        --ignore-file .local_src_ignore \
        --ignore-file .xxx_ignore \
        XXX
}

xxx_check() {
    # capture exit code without triggering errexit
    xxx && rg=$? || rg=$?

    if [ $rg -ne 1 ]; then
        # Matches found, or error
        printf "\n>> XXX annotations found!\n"
        return 1
    else
        return 0
    fi
}

lint() {
    cargo clippy -F dev "$@"
}

lint_check() {
    lint "$@" >& /dev/null || {
        printf "\n>> Trapped lint!\n"
        return 1;
    }
}

fmt() {
    cargo fmt "$@"
}

fmt-check() {
    fmt --check --quiet "$@" || {
        printf "\n>> Formatting required!\n"
        return 1;
    }
}

ci_check() {
    # xxx_check
    # lint_check
    fmt-check
}

ci_check
