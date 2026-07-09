#!/bin/bash

# These only work if this script is run in the SDK's top-level (workspace) directory.
SDK_DIR="$(realpath ".")"
TARGET_DIR="$(realpath "target")"

### BUILDING ###

cargo() {
    if [ -v CARGO_UNLOCK ] && [ -n "$CARGO_UNLOCK" ]; then
        command cargo "$@"
    else
        command cargo --locked "$@"
    fi
}

CONTRACT="${TARGET_DIR}/wasm32-unknown-unknown/release"
OPTIMIZED="${TARGET_DIR}/optimized"
RV32_TARGET="riscv32im-unknown-none-elf"
RV32_RELEASE="${TARGET_DIR}/${RV32_TARGET}/release"

# Wasm features the pinned nightly emits, enabled explicitly for every wasm-opt
# invocation. Older binaryen builds don't auto-detect them from the
# target_features section and reject e.g. i32.extend8_s; all of these are
# ABI-neutral, stable, and already accepted by the runtime.
WASM_OPT_FEATURES="--enable-bulk-memory --enable-sign-ext --enable-mutable-globals --enable-nontrapping-float-to-int"

build_contracts() {
    local workspace_dir
    workspace_dir="$1"; shift
    local packages_dir
    packages_dir="${1:-$workspace_dir}"; shift 2>/dev/null || true

    local package
    for package in $(ls_contract_packages "$packages_dir") ; do
        build_contract "$workspace_dir" "$package" || {
            fail "Failed building contract $package"
            return 1
        }
    done
}

build_contract() {
    local workspace_dir
    workspace_dir="$1"; shift
    local package
    package="$1"; shift

    info "BUILD $package"

    build_wasm "$workspace_dir" "$package" --release &&
    make_optimized "$package" &&
    finalize_contract_spec "$package"
}

build_wasm() {
    local workspace_dir
    workspace_dir="$1"; shift
    local package
    package="$1"; shift

    (
        cd "$workspace_dir" &&
        cargo --config ".cargo/contract.toml" \
            build -p "$package" -F entrypoints "$@"
    )
}

make_optimized() {
    local package
    package="$1"; shift

    local contract optimized
    # Cargo converts hyphens to underscores in output filenames
    local wasm_name="${package//-/_}"
    contract="${CONTRACT}/${wasm_name}.wasm"
    optimized="${OPTIMIZED}/${wasm_name}.wasm"

    mkdir -p "$OPTIMIZED" &&

    # debugging="-g" # don't strip symbols?
    debugging=""

    # further compress the wasm (anecdotally, wasm-opt -Os and -Oz do only
    # very slightly better)
    #
    # Enable the wasm features the pinned nightly emits explicitly, rather than
    # relying on wasm-opt's binaryen version auto-detecting them from the
    # target_features section (older binaryen builds don't, and reject e.g.
    # i32.extend8_s). All are ABI-neutral, stable, and already accepted by the
    # runtime.
    wasm-opt -O3 $debugging $WASM_OPT_FEATURES -o "${optimized}" "${contract}"
}

# rv32 bin-wrapper packages carry a `disabled` marker so the wasm build loop
# (ls_contract_packages) skips them; they are discovered here by the *-rv32
# directory-name convention instead.
ls_rv32_packages() {
    local contracts_dir
    contracts_dir="$1"; shift

    local contract
    for contract in "$contracts_dir"/*-rv32 ; do
        [ -d "$contract" ] || continue;
        [ -f "$contract"/Cargo.toml ] || continue;

        basename "$contract"
    done
}

build_rv32() {
    local workspace_dir
    workspace_dir="$1"; shift
    local package
    package="$1"; shift

    info "BUILD(rv32) $package"

    (
        cd "$workspace_dir" &&
        # build-std on the command line, exactly like hogd's build_risc.sh:
        # the rv32 rustflags (example-contracts/.cargo/config.toml) use
        # panic=immediate-abort + target features the prebuilt core lacks.
        # NOT in [unstable] config — that would break host `cargo test`
        # runs from this cwd (build-std requires --target).
        cargo build -Zbuild-std=core,alloc \
            -p "$package" -F entrypoints --release --target "$RV32_TARGET"
    )
}

check_rv32_dispatch() {
    local elf
    elf="$1"; shift

    bash "${SDK_DIR}/scripts/check-rv32-dispatch.sh" "$elf"
}

check_rv32_strings() {
    # Same invariant check-wasm-size.sh enforces for wasm: production rv32
    # builds (panic=immediate-abort, strip=symbols) must embed no .rs file
    # paths. No allow-list — no rv32 contract builds with `debugging` today;
    # if one ever does, promote this to a script sharing check-wasm-size.sh's
    # allow-list logic rather than forking it here.
    info "Checking rv32 binaries for embedded path strings"

    local elf rs_paths
    # Root workspace and example-contracts share target/, so a manual
    # release-profile rv32 build of a non-contract bin (e.g. the erc20-if
    # spec carrier) can land in this dir too. Scan only contract wrapper
    # bins, which by convention are named *_rv32 (see eg_build_rv32_all).
    for elf in "${RV32_RELEASE}"/*_rv32; do
        [ -f "$elf" ] && [ -x "$elf" ] || continue
        rs_paths=$(strings "$elf" | grep -E '\.rs$' || true)
        if [ -n "$rs_paths" ]; then
            printf '%s\n' "$rs_paths" | sed 's/^/  /'
            fail "$(basename "$elf") contains embedded .rs path strings"
            return 1
        fi
        echo "OK:   $(basename "$elf")"
    done
}

eg_build_rv32_all() {
    local package elf
    for package in $(ls_rv32_packages "${SDK_DIR}/example-contracts"); do
        build_rv32 "${SDK_DIR}/example-contracts" "$package" || {
            fail "Failed building rv32 contract $package"
            return 1
        }
        # Bin names are NOT auto-converted by cargo: *-rv32 wrapper packages
        # must declare `[[bin]] name` as the underscored package name (see
        # counter-rv32/Cargo.toml), which is what this path derivation assumes.
        elf="${RV32_RELEASE}/${package//-/_}"
        check_rv32_dispatch "$elf" || return 1
    done
    check_rv32_strings
}

finalize_contract_spec() {
    local package
    package="$1"; shift

    local wasm_name optimized json_out
    wasm_name="${package//-/_}"
    optimized="${OPTIMIZED}/${wasm_name}.wasm"
    json_out="${OPTIMIZED}/${wasm_name}.spec.json"

    info "SPEC $package"

    (
        cd "$SDK_DIR" &&
        cargo run --quiet -p groundhog-spec-tool -- \
            rewrite \
            --wasm "${optimized}" \
            --json-out "${json_out}"
    )
}

eg_build() {
    local package
    package="$1"; shift

    build_contract "${SDK_DIR}/example-contracts" "${package}"
}

eg_build_all() {
    build_contracts "${SDK_DIR}/example-contracts"
}

assert_file_contains() {
    local path
    path="$1"; shift
    local needle
    needle="$1"; shift

    rg -F --quiet "$needle" "$path" || {
        fail "Expected $path to contain $needle"
        return 1
    }
}

assert_wasm_has_final_spec_only() {
    local path
    path="$1"; shift
    local dump
    dump="$(wasm-objdump -x "$path")" || return 1

    printf '%s\n' "$dump" | rg -F --quiet "groundhogspecv0" || {
        fail "Expected $path to contain groundhogspecv0"
        return 1
    }
    if printf '%s\n' "$dump" | rg -F --quiet "groundhogspecrawv0"; then
        fail "Expected $path to omit groundhogspecrawv0"
        return 1
    fi
}

check_contract_spec_artifacts() {
    info "CHECK-SPECS"

    local optimized
    optimized="${TARGET_DIR}/optimized"

    local free_fn_json free_fn_wasm imported_json multi_json multi_wasm spec_named_json genesis_json genesis_wasm erc20_json swap_json
    free_fn_json="${optimized}/free_fn_unnamed.spec.json"
    free_fn_wasm="${optimized}/free_fn_unnamed.wasm"
    imported_json="${optimized}/imported_error_callee.spec.json"
    multi_json="${optimized}/multi_imported_traits.spec.json"
    multi_wasm="${optimized}/multi_imported_traits.wasm"
    spec_named_json="${optimized}/spec_named_types.spec.json"
    genesis_json="${optimized}/genesis_deployer.spec.json"
    genesis_wasm="${optimized}/genesis_deployer.wasm"
    erc20_json="${optimized}/erc20_raw_storage.spec.json"
    swap_json="${optimized}/swap.spec.json"

    assert_file_contains "$free_fn_json" '"format":"gh_contract_spec_v1"' &&
    assert_file_contains "$free_fn_json" '"name":"_unnamed_interface"' &&
    assert_file_contains "$free_fn_json" '"version":"0.4.0"' &&
    assert_wasm_has_final_spec_only "$free_fn_wasm" &&

    assert_file_contains "$imported_json" '"name":"ImportedErrorCallee"' &&
    assert_file_contains "$imported_json" '"ImportedError"' &&

    assert_file_contains "$multi_json" '"name":"ExplicitNamedInterface"' &&
    assert_file_contains "$multi_json" '"name":"VersionedCallee"' &&
    assert_file_contains "$multi_json" '"version":"1.2.3"' &&
    assert_wasm_has_final_spec_only "$multi_wasm" &&

    assert_file_contains "$spec_named_json" '"type_defs":[' &&
    assert_file_contains "$spec_named_json" '"name":"SpecNamedTypesContract"' &&
    assert_file_contains "$spec_named_json" '"Payload"' &&
    assert_file_contains "$spec_named_json" '"Choice"' &&
    assert_file_contains "$spec_named_json" '"Envelope"' &&
    assert_file_contains "$spec_named_json" '"name":"echo_blob"' &&
    assert_file_contains "$spec_named_json" '"kind":"vec","element":{"kind":"u8"}' &&
    assert_file_contains "$spec_named_json" '"name":"echo_hash"' &&
    assert_file_contains "$spec_named_json" '"kind":"array","len":32,"element":{"kind":"u8"}' &&
    assert_file_contains "$spec_named_json" '"name":"echo_blob_envelope"' &&
    assert_file_contains "$spec_named_json" '"name":"PayloadEvent"' &&
    assert_file_contains "$spec_named_json" '"name":"AuditEvent"' &&

    assert_file_contains "$erc20_json" '"events":[' &&
    assert_file_contains "$erc20_json" '"name":"TransferEvent"' &&
    assert_file_contains "$erc20_json" '"name":"ApprovalEvent"' &&
    assert_file_contains "$erc20_json" '"name":"from","type":{"kind":"address"}' &&

    assert_file_contains "$swap_json" '"name":"MintEvent"' &&
    assert_file_contains "$swap_json" '"name":"BurnEvent"' &&
    assert_file_contains "$swap_json" '"name":"SwapEvent"' &&
    assert_file_contains "$swap_json" '"name":"SyncEvent"' &&

    # Regression guard: method specs are sourced from the ISwapV2Pair *trait*,
    # not the impl. The impl names the first `initialize` arg `_factory` (unused
    # marker); the trait names it `factory`. If method specs ever revert to
    # being impl-sourced, the same-crate trait+impl build fails with
    # `conflicting method definitions`, or this assertion catches `_factory`
    # leaking into the published interface spec.
    assert_file_contains "$swap_json" '"name":"initialize","args":[{"name":"factory"' &&

    assert_file_contains "$genesis_json" '"name":"GenesisDeployer"' &&
    assert_file_contains "$genesis_json" '"name":"deploy"' &&
    assert_file_contains "$genesis_json" '"name":"deploy_and_invoke"' &&
    assert_wasm_has_final_spec_only "$genesis_wasm"
}

itc_build() {
    package="$1"; shift

    build_contract "${SDK_DIR}/integration_test" "${package}"
}

itc_build_all() {
    itc_build fibonacci &&
    build_contracts "${SDK_DIR}/integration_test" "${SDK_DIR}/integration_test/contracts"
}

# Regression guard: the logging-test workspace unconditionally enables
# sdk/logging, which makes the codegen emit debug_log! calls in client
# stubs.  This catches use-after-move bugs for non-Copy by-value params
# (e.g. BlockHeight) that only surface under feature unification.
logging_test_build() {
    info "BUILD logging-test (wasm + spec)"

    (
        cd "${SDK_DIR}/logging-test" &&
        cargo check -p htlc-logging --target wasm32-unknown-unknown
    )
}

### LINT ###

lint_contracts() {
    local workspace_dir
    workspace_dir="$1"; shift
    local packages_dir
    packages_dir="${1:-$workspace_dir}"; shift 2>/dev/null || true

    local package
    for package in $(ls_contract_packages "$packages_dir") ; do
        lint_contract "$workspace_dir" "$package" || {
            fail "Failed linting contract $package"
            return 1
        }
    done
}

lint_contract() {
    local workspace_dir
    workspace_dir="$1"; shift
    local package
    package="$1"; shift

    info "LINT $package"

    (
        cd "$workspace_dir" &&

        cargo --config ".cargo/contract.toml" \
            clippy -p "$package" \
                --release \
                --target wasm32-unknown-unknown \
                -- -Dwarnings &&

        cargo clippy -F dev -p "$package"
    )
}

### TESTING ###

test_package() {
    cargo test &&
    cargo test -p sdk -Fstd --tests &&
    cargo test -p groundhog-erc20-metadata-if --all-targets &&
    cargo test -p groundhog-spec-tool --all-targets &&
    cargo test -p groundhog-test-vectors &&
    cargo test -p vector-tests --tests &&
    true
}

test_contract() {
    local workspace_dir
    workspace_dir="$1"; shift
    local package
    package="$1"; shift

    info "TEST $package"
    (
        cd "$workspace_dir" &&
        cargo test -p "$package" "$@"
    )
}

eg_test() {
    local package
    package="$1"; shift

    test_contract "example-contracts" "$package" "$@"
}

eg_test_all() {
    # Note: Some of these tests require their associated wasms to be compiled
    # first.

    packages_dir="example-contracts"

    for package in $(ls_contract_packages "$packages_dir") ; do
        eg_test "$package" || {
            fail "Failed testing contract $package"
            return 1
        }
    done
}

itc_test() {
    local package
    package="$1"; shift

    test_contract "integration_test" "$package"
}

itc_test_all() {
    (
        cd integration_test &&
        cargo test \
            --workspace  \
            "$@" -- --show-output
    )
}

### BOTTLE TEST ###

BOTTLE_TEST_RAW="${TARGET_DIR}/wasm32-unknown-unknown/bottle_test"
BOTTLE_TEST="${TARGET_DIR}/bottle-test"

bottle_test_contract() {
    # NOTE: This requires that the contract's wasm has already been built

    local workspace_dir package
    workspace_dir="$1"; shift
    package="$1"; shift

    info "BOTTLE-TEST $package"

    build_wasm "$workspace_dir" "$package" \
        --profile="bottle_test" \
        --features="bottle-test" \
        -Z unstable-options \
        &&

    local wasm_name="${package//-/_}" &&
    mkdir -p "$BOTTLE_TEST" &&
    wasm-opt -O3 $WASM_OPT_FEATURES \
        -o "$BOTTLE_TEST/${wasm_name}.wasm" \
        "$BOTTLE_TEST_RAW/${wasm_name}.wasm" &&

    cargo run -p bottle-test --profile dev "$package"
}

has_bottle_test() {
    local contract cargo_toml
    contract="$1"; shift
    cargo_toml="$contract/Cargo.toml"

    grep -s "bottle-test" "$cargo_toml" >/dev/null
}

bottle_test_contracts() {
    local workspace_dir
    workspace_dir="$1"; shift
    local packages_dir
    packages_dir="${1:-$workspace_dir}"; shift 2>/dev/null || true

    local package
    for package in $(ls_contract_packages "$packages_dir") ; do
        if has_bottle_test "$packages_dir/$package"; then
            [ ! -f "$packages_dir/$package/disabled-bottle" ] || continue

            bottle_test_contract "$workspace_dir" "$package" "$@" ||
                return 1
        fi
    done
}

ls_contract_packages() {
    local contracts_dir
    contracts_dir="$1"; shift

    local contract
    for contract in "$contracts_dir"/* ; do
        [ -d "$contract" ] || continue;
        [ -f "$contract"/Cargo.toml ] || continue;
        [ ! -f "$contract"/disabled ] || continue;

        c="$(basename "$contract")"

        # Skip cargo target directories
        [ "$c" != "target" ] || continue;

        echo "$c"
    done
}

eg_bottle_test_all() {
    bottle_test_contracts "example-contracts"
}

itc_bottle_test_all() {
    bottle_test_contracts "integration_test" "integration_test/contracts"
}

build_docs() {
    RUSTDOCFLAGS="--deny warnings --cfg docsrs" \
        cargo doc --features=dev "$@"
}

skip() {
    local var="SKIP_$1"
    [ "${!var:-}" = "1" ]
}

check_all() {
    { skip DOCS || build_docs; } &&
    { skip SDK_TESTS || test_package; } &&

    { skip EXAMPLES || {
        { skip BUILD_EXAMPLES || eg_build_all; } &&
        { skip RV32_EXAMPLES || eg_build_rv32_all; } &&
        check_contract_spec_artifacts &&
        { skip TEST_EXAMPLES || eg_test_all; } &&
        eg_bottle_test_all &&
        { skip DOMAIN_COLLISION || bash "${SDK_DIR}/scripts/check-domain-collision-detected.sh"; } &&
        { skip REGISTRY_STRIPPED || bash "${SDK_DIR}/scripts/check-storage-registry-stripped.sh"; }
    }; } &&

    { skip INTEGRATION || {
        itc_build_all &&
        itc_test_all &&
        itc_bottle_test_all
    }; } &&

    { skip LOGGING || logging_test_build; } &&

    info "All tests succeeded." ||
    fail "Some tests failed"
}

### Other dev utils

find_contract_config() {
    local dir="$1"
    while [ ! -f "$dir/.cargo/contract.toml" ] && [ "$dir" != "/" ]; do
        dir="$(dirname "$dir")"
    done
    echo "$dir/.cargo/contract.toml"
}

tree_contract() {
    local contract
    contract=$(realpath "$1"); shift
    local config
    config="$(find_contract_config "$contract")"

    (
        cd "$contract" &&
        cargo --config "$config" \
            tree \
            --target "wasm32-unknown-unknown" \
            --edges no-proc-macro,no-build,no-dev \
            -Z build-std=core,alloc \
            -Z build-std-features=std/optimize_for_size \
            "$@"
    )
}

expand_contract() {
    local contract
    contract=$(realpath "$1"); shift
    local config
    config="$(find_contract_config "$contract")"

    (
        cd "$contract" &&
        cargo --config "$config" \
            expand \
            --features "entrypoints" \
            --target "wasm32-unknown-unknown" \
            --color=always \
        #
    )
}

contract_entrypoints() {
    local package
    package=$(basename "$1"); shift

    wasm="${CONTRACT}/${package}.wasm"

    ls -l "$wasm" &&
    wasm-objdump -j Export -x "$wasm"
}

### Quality checks

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
    which rg >& /dev/null || {
        echo "Can't find rg"
        return 0;
    }

    # capture exit code without triggering errexit
    xxx && rg=$? || rg=$?

    if [ $rg -ne 1 ]; then
        # Matches found, or error
        fail "XXX annotations found!"
    else
        return 0
    fi
}

lint() {
    cargo clippy -F dev --lib &&
    lint_contracts "example-contracts" &&
    lint_contracts "integration_test" "contracts"
}

lint_check() {
    lint || fail "Trapped lint!"
}

fmt() {
    cargo fmt "$@"
}

fmt_check() {
    fmt --check --quiet ||
        fail "Formatting required!"
}

ci_check() {
    check_all &&
    { skip WASM_STRINGS || check_wasm_strings; } &&
    { skip QUALITY || check_quality; } &&
    info "CI clean!" ||
    fail "CI failed!"
}

check_quality() {
    { skip LINT || lint_check; } &&
    { skip FMT || fmt_check; } &&
    xxx_check &&
    true
}

check_wasm_strings() {
    info "Checking WASM binaries for embedded path strings"
    bash "${SDK_DIR}/scripts/check-wasm-size.sh" "$CONTRACT"
}

info() {
    printf "\n-- %s\n" "$1" >&2
}

warn() {
    printf "\n>> %s\n" "$1" >&2
}

fail() {
    printf "\n!! %s\n" "$1" >&2
    return 1
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    set -u

    action="${1:-}"
    if [ $# -gt 0 ]; then
        shift
    fi

    case "$action" in
        expand)
            expand_contract "$1"
            ;;
        lint)
            if [ $# -gt 0 ]; then
                dir=$(dirname "$1")
                pkg=$(basename "$1")
                shift

                lint_contract "$dir" "$pkg"
            else
                lint
            fi
            ;;
        [xX][xX][xX])
            xxx
            ;;
        ci)
            ci_check
            ;;
        qual)
            check_quality
            ;;
        ls-enabled)
            ls_contract_packages "$1"
            ;;
        build)
            dir=$(dirname "$1")
            pkg=$(basename "$1")

            build_contract "$dir" "$pkg"
            ;;
        build-examples)
            eg_build_all
            ;;
        build-examples-rv32)
            eg_build_rv32_all
            ;;
        build-itc)
            itc_build_all
            ;;
        test)
            if [ $# -gt 0 ]; then
                dir=$(dirname "$1")
                pkg=$(basename "$1")
                shift

                test_contract "$dir" "$pkg" "$@"
            else
                test_all
            fi
            ;;
        bottle-test)
            bottle_test_contract "$@"
            ;;
        test-examples)
            eg_test_all
            ;;
        test-itc)
            itc_test_all "$@"
            ;;
        test-spec)
            test_spec
            ;;
        run-bottle-tests)
            eg_bottle_test_all &&
                itc_bottle_test_all
            ;;
        entrypoints)
            contract_entrypoints "$@"
            ;;
        strings)
            check_wasm_strings
            ;;
        tree)
            tree_contract "$1"
            ;;
        build-docs)
            build_docs
            ;;
        open-docs)
            build_docs --open
            ;;
        rg)
            rg -i --ignore-file .local_src_ignore "$@"
            ;;
        *)
            fail "Sorry, didn't understand action '$action'."
            ;;
    esac
fi
