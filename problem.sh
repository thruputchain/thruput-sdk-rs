eg_test() {
    package="$1"; shift

    (cd example-contracts &&
        cargo test -p "$package" "$@" -- --show-output)
}

SDK_TEST_ENABLE_SYSCALL_LOGGING=yes TEST_FFI_TRACE=1 eg_test caller --test two
