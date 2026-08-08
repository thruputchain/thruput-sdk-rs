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

    # --locked matches the cargo() wrapper's default for the CLI's inner
    # cargo invocation; CARGO_UNLOCK opts out, same as everywhere else.
    # ${VAR:-} (not [ -v ]) so macOS bash 3.2 parses it; same semantics:
    # opt out of --locked only when CARGO_UNLOCK is set non-empty.
    local locked="--locked"
    if [ -n "${CARGO_UNLOCK:-}" ]; then
        locked=""
    fi

    (
        cd "$workspace_dir" &&
        # The groundhog CLI compiles the ELF (honoring this workspace's
        # .cargo/config.toml rv32 rustflags) and lowers it in-process,
        # writing target/groundhog/<snake>.{rv32a,rv32.json,spec.json,spec.xdr}.
        # No -F here: the CLI itself passes --no-default-features and the
        # package's entrypoints (or rv32-entrypoints) feature — see the
        # build_flags it records in the .rv32.json sidecar.
        #
        # build-std passes through as trailing cargo args, exactly like
        # hogd's build_risc.sh: the rv32 rustflags use panic=immediate-abort
        # + target features the prebuilt core lacks. NOT in [unstable]
        # config — that would break host `cargo test` runs from this cwd
        # (build-std requires --target).
        #
        # Build unstripped (env overrides the config profile's
        # strip="symbols"): the symtab does not change .text/.rodata/.data
        # sizes, and it lets the dispatch check use its strong symbol path
        # and check_rv32_mem_symbols assert the compiler_builtins
        # mem-override held. The deployed artifact is the lowered .rv32a,
        # which carries no symtab regardless.
        CARGO_PROFILE_RELEASE_STRIP=false \
        groundhog build --target rv32 -p "$package" -- $locked -Zbuild-std=core,alloc
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
    #
    # Takes package names as args; scans exactly those ELFs, not a directory
    # glob: the release dir is shared with root-workspace rv32 builds and
    # can hold stale ELFs from renamed or removed packages.
    info "Checking rv32 binaries for embedded path strings"

    local package elf rs_paths
    for package in "$@"; do
        elf="${RV32_RELEASE}/${package//-/_}"
        [ -f "$elf" ] || {
            fail "missing rv32 ELF $elf (run build-examples-rv32 / build-integration-rv32 first)"
            return 1
        }
        rs_paths=$(strings "$elf" | grep -E '\.rs$' || true)
        if [ -n "$rs_paths" ]; then
            printf '%s\n' "$rs_paths" | sed 's/^/  /'
            fail "$(basename "$elf") contains embedded .rs path strings"
            return 1
        fi
        echo "OK:   $(basename "$elf")"
    done
}

check_rv32_mem_symbols() {
    # The rv32 runtime overrides compiler_builtins' weak memcpy/memmove/memset
    # (crates/rv32-runtime/src/mem.rs). Assert per contract ELF that the
    # override held: our strong memcpy is present and no compiler_builtins mem
    # symbol survived. Catches a toolchain/compiler_builtins bump silently
    # dropping the override (size regression) — and, worse, a build where
    # #![no_builtins] stopped guarding the bodies against self-recursion
    # (recursion would resurrect the compiler_builtins symbols' caller graph).
    local elf
    elf="$1"; shift

    local readelf
    readelf="${READELF:-$(command -v llvm-readelf || command -v readelf || true)}"
    [ -n "$readelf" ] || {
        fail "no llvm-readelf or readelf on PATH (or set READELF)"
        return 1
    }

    local syms
    syms=$("$readelf" -sW "$elf") || {
        fail "readelf failed reading symbols from $elf"
        return 1
    }

    # A WEAK mem* definition means a compiler_builtins fallback won the link
    # for that symbol (ours are GLOBAL). Mangling-independent, so it holds
    # across compiler_builtins refactors, and it covers memmove/memset too.
    if printf '%s\n' "$syms" | grep -qE ' FUNC +WEAK .* (memcpy|memmove|memset)$'; then
        fail "$(basename "$elf"): WEAK mem symbol linked (compiler_builtins fallback won)"
        return 1
    fi
    # Belt-and-braces for the current mangling: no compiler_builtins mem
    # helpers may survive alongside the overrides.
    if printf '%s\n' "$syms" | grep -q "compiler_builtins3mem"; then
        fail "$(basename "$elf"): compiler_builtins mem symbols present (override not applied)"
        return 1
    fi

    if printf '%s\n' "$syms" | grep -qE ' FUNC +(GLOBAL|LOCAL) .* (memcpy|memmove|memset)$'; then
        # Strong (non-WEAK) definitions won. GLOBAL is the common case;
        # LOCAL HIDDEN appears when LTO internalizes our #[no_mangle] defs
        # (seen with read_only_test) — still ours, since a fallback winning
        # would trip the WEAK or compiler_builtins checks above.
        echo "OK:   $(basename "$elf") mem override held"
    else
        # No mem symbols at all: the contract's code graph never references a
        # mem intrinsic, so --gc-sections dropped our (unreferenced) overrides
        # along with any fallback. Nothing linked, nothing to assert.
        echo "OK:   $(basename "$elf") links no mem intrinsics (gc'd)"
    fi
}

eg_build_rv32_all() {
    require_groundhog_cli || return 1

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
        check_rv32_mem_symbols "$elf" || return 1
    done
    check_rv32_strings $(ls_rv32_packages "${SDK_DIR}/example-contracts") &&
    check_rv32_contract_spec_artifacts
}

# integration_test rv32 contracts are NOT *-rv32 wrapper packages — the rv32
# bin wrapper is each contract's own src/main.rs. Auto-discovered: the wasm
# contract set (dirs with Cargo.toml minus `disabled`, same as the wasm build
# loop) narrowed to packages that declare a [[bin]]. Declaring the wrapper
# bin IS the rv32 enablement; raw wasm-ABI contracts (hand-written
# export_name entrypoints, no bin — e.g. sha256_raw) are excluded by
# construction until someone ports them. The metric scripts
# (scripts/metrics/sdk_codesizes_rv32*.sh) derive the same set — keep the
# rules in sync.
ls_itc_rv32_packages() {
    local contracts_dir="${SDK_DIR}/integration_test/contracts"
    local package
    for package in $(ls_contract_packages "$contracts_dir"); do
        grep -q '^\[\[bin\]\]' "$contracts_dir/$package/Cargo.toml" || continue
        echo "$package"
    done
}

itc_build_rv32_all() {
    require_groundhog_cli || return 1
    # Discovery yielding nothing means the tree moved under us (dir renamed,
    # bins dropped) — fail rather than silently build zero contracts.
    [ -n "$(ls_itc_rv32_packages)" ] || {
        fail "no rv32-capable integration_test contracts discovered"
        return 1
    }

    local package elf
    for package in $(ls_itc_rv32_packages); do
        build_rv32 "${SDK_DIR}/integration_test" "$package" || {
            fail "Failed building rv32 contract $package"
            return 1
        }
        elf="${RV32_RELEASE}/${package//-/_}"
        check_rv32_dispatch "$elf" || return 1
        check_rv32_mem_symbols "$elf" || return 1
    done
    check_rv32_strings $(ls_itc_rv32_packages)
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

assert_file_lacks() {
    local path
    path="$1"; shift
    local needle
    needle="$1"; shift

    if rg -F --quiet "$needle" "$path"; then
        fail "Expected $path to lack $needle"
        return 1
    fi
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

    local free_fn_json free_fn_wasm imported_json multi_json multi_wasm spec_named_json genesis_json genesis_wasm erc20_json swap_json signer_callee_json erc1271_wallet_json event_shaking_json typedef_shaking_json iface_conflict_json
    signer_callee_json="${optimized}/signer_callee.spec.json"
    erc1271_wallet_json="${optimized}/erc1271_wallet.spec.json"
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
    event_shaking_json="${optimized}/event_shaking_subset.spec.json"
    typedef_shaking_json="${optimized}/typedef_shaking_caller.spec.json"
    iface_conflict_json="${optimized}/iface_conflict_caller.spec.json"

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
    assert_file_lacks "$spec_named_json" '"name":"GhostEvent"' &&
    assert_file_lacks "$spec_named_json" '"name":"UnimplementedInterface"' &&

    # Cross-crate event shaking: event_shaking_subset implements only the
    # `Adopted` trait from event_shaking_if; `Orphan` is never implemented.
    # The spec JSON is minified (no spaces), so needle matching is exact.
    # "name":"Orphan"  ends with a closing quote so it cannot false-match
    # "name":"OrphanEvent" (which has 'E' after "Orphan", not '"').
    # AdoptedPayload asserts positive typedef retention; OrphanPayload asserts
    # typedef shaking (both directions covered by a single fixture).
    assert_file_contains "$event_shaking_json" '"name":"Adopted"' &&
    assert_file_contains "$event_shaking_json" '"name":"AdoptedEvent"' &&
    assert_file_contains "$event_shaking_json" '"name":"AdoptedPayload"' &&
    assert_file_lacks "$event_shaking_json" '"name":"Orphan"' &&
    assert_file_lacks "$event_shaking_json" '"name":"OrphanEvent"' &&
    assert_file_lacks "$event_shaking_json" '"name":"OrphanPayload"' &&

    # Typedef-collision shaking: typedef_shaking_caller links two "versions"
    # of a market interface crate (typedef_shaking_if_v1/v2) purely to call
    # them, implementing neither trait. Each crate defines a differently
    # shaped `SharedConfig`; both defs co-travel into the raw section but are
    # unreachable, so the conflict must not fail the build (pre-fix this
    # build died with ConflictingTypeDef) and nothing from the market
    # interfaces may reach the final spec.
    assert_file_contains "$typedef_shaking_json" '"name":"Caller"' &&
    assert_file_lacks "$typedef_shaking_json" '"name":"SharedConfig"' &&
    assert_file_lacks "$typedef_shaking_json" '"name":"MarketV1"' &&
    assert_file_lacks "$typedef_shaking_json" '"name":"MarketV2"' &&

    # Interface-metadata conflict shaking: iface_conflict_caller links two
    # interface crates whose `Token` traits collide on the same interface key
    # (default name = trait ident, default version = workspace crate version)
    # with different emits metadata, implementing neither. Both candidates are
    # shaken, so the disagreement must not fail the build (pre-fix this build
    # died with ConflictingInterfaceEmits) and neither Token nor PingEvent may
    # reach the final spec.
    assert_file_contains "$iface_conflict_json" '"name":"ConflictCaller"' &&
    assert_file_lacks "$iface_conflict_json" '"name":"Token"' &&
    assert_file_lacks "$iface_conflict_json" '"name":"PingEvent"' &&

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
    assert_wasm_has_final_spec_only "$genesis_wasm" &&

    # `Signed<T>` args/returns spec as signed_token nodes carrying the
    # preimage type (they consume no calldata bytes), never as u32 handles;
    # `Signed<Opaque>` is an erased (null) preimage.
    assert_file_contains "$signer_callee_json" '"name":"auth","type":{"kind":"signed_token","preimage":{"kind":"named","name":"TransferAuth"}}' &&
    assert_file_contains "$signer_callee_json" '"name":"TransferAuth"' &&
    assert_file_contains "$erc1271_wallet_json" '"kind":"vec","element":{"kind":"signed_token","preimage":null}'
}

# Negative spec fixture: typedef_collision_reachable implements MarketV1
# (anchoring it, so v1's SharedConfig is reachable from the retained spec)
# while also linking typedef_shaking_if_v2, whose unimplemented candidate
# contributes a differently-shaped SharedConfig def. The wasm build succeeds;
# the spec rewrite must FAIL with a typedef conflict — pinning that the
# reachable-conflict error survived the unreachable-conflict relaxation.
check_typedef_conflict_detected() {
    info "CHECK-SPEC-CONFLICT-DETECTED"

    build_wasm "${SDK_DIR}/example-contracts" typedef_collision_reachable \
        --release || return 1

    local out
    if out=$( (
        cd "$SDK_DIR" &&
        cargo run --quiet -p groundhog-spec-tool -- \
            rewrite \
            --wasm "${CONTRACT}/typedef_collision_reachable.wasm" \
            --json-out "${CONTRACT}/typedef_collision_reachable.spec.json"
    ) 2>&1); then
        fail "Expected spec rewrite of typedef_collision_reachable to fail with a typedef conflict, but it succeeded"
        return 1
    fi
    echo "$out" | rg -F --quiet \
        "conflicting contract type definitions for SharedConfig" || {
        fail "Expected a SharedConfig typedef conflict from typedef_collision_reachable, got: $out"
        return 1
    }
}

# Negative spec fixture: iface_conflict_impl implements iface_conflict_if_a's
# Token (anchoring the shared Token interface key) while also linking
# iface_conflict_if_b, whose colliding Token candidate carries different
# emits metadata. The wasm build succeeds; the spec rewrite must FAIL with
# an emits-metadata conflict — pinning that the anchored-key conflict error
# survived the unanchored-conflict relaxation.
check_iface_conflict_detected() {
    info "CHECK-IFACE-CONFLICT-DETECTED"

    build_wasm "${SDK_DIR}/example-contracts" iface_conflict_impl \
        --release || return 1

    local out
    if out=$( (
        cd "$SDK_DIR" &&
        cargo run --quiet -p groundhog-spec-tool -- \
            rewrite \
            --wasm "${CONTRACT}/iface_conflict_impl.wasm" \
            --json-out "${CONTRACT}/iface_conflict_impl.spec.json"
    ) 2>&1); then
        fail "Expected spec rewrite of iface_conflict_impl to fail with an emits-metadata conflict, but it succeeded"
        return 1
    fi
    echo "$out" | rg -F --quiet \
        "conflicting emits metadata for interface Token" || {
        fail "Expected a Token emits-metadata conflict from iface_conflict_impl, got: $out"
        return 1
    }
}

# Asserts on rv32-pipeline-produced spec JSONs written to target/groundhog/.
# Called at the end of eg_build_rv32_all (not from check_all) so it only runs
# when the rv32 build succeeds and is not skipped via SKIP_RV32_EXAMPLES.
# NOTE: assumes the rv32 build loop ran in the same invocation — target/groundhog
# is never cleaned automatically, so a stale spec from a previous run could
# otherwise satisfy these checks without the current code being re-exercised.
check_rv32_contract_spec_artifacts() {
    info "CHECK-SPECS(rv32)"

    local groundhog_dir
    groundhog_dir="${TARGET_DIR}/groundhog"

    # Same needles as the wasm block for event_shaking_subset; the rv32 spec
    # JSON is also minified (no spaces), so the same false-positive reasoning
    # applies: "name":"Orphan" (closing quote after n) cannot match
    # "name":"OrphanEvent" (E comes before the closing quote).
    # AdoptedPayload asserts positive typedef retention; OrphanPayload asserts
    # typedef shaking (both directions covered by a single fixture).
    local rv32_event_shaking_json
    rv32_event_shaking_json="${groundhog_dir}/event_shaking_subset_rv32.spec.json"

    assert_file_contains "$rv32_event_shaking_json" '"name":"Adopted"' &&
    assert_file_contains "$rv32_event_shaking_json" '"name":"AdoptedEvent"' &&
    assert_file_contains "$rv32_event_shaking_json" '"name":"AdoptedPayload"' &&
    assert_file_lacks "$rv32_event_shaking_json" '"name":"Orphan"' &&
    assert_file_lacks "$rv32_event_shaking_json" '"name":"OrphanEvent"' &&
    assert_file_lacks "$rv32_event_shaking_json" '"name":"OrphanPayload"'
}

itc_build() {
    package="$1"; shift

    build_contract "${SDK_DIR}/integration_test" "${package}"
}

itc_build_all() {
    # fibonacci is in the loop (alphabetically before fibonacci_call, whose
    # tests consume its wasm); it no longer needs an explicit first build.
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
        check_typedef_conflict_detected &&
        check_iface_conflict_detected &&
        { skip TEST_EXAMPLES || eg_test_all; } &&
        eg_bottle_test_all &&
        { skip DOMAIN_COLLISION || bash "${SDK_DIR}/scripts/check-domain-collision-detected.sh"; } &&
        { skip REGISTRY_STRIPPED || bash "${SDK_DIR}/scripts/check-storage-registry-stripped.sh"; }
    }; } &&

    { skip INTEGRATION || {
        itc_build_all &&
        { skip RV32_INTEGRATION || itc_build_rv32_all; } &&
        itc_test_all &&
        itc_bottle_test_all
    }; } &&

    { skip LOGGING || logging_test_build; } &&

    # now that Cargo.lock files have been updated:
    cargo run -p version-check &&

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

require_groundhog_cli() {
    # rv32 contract builds run through the groundhog CLI. CI installs the
    # exact pin (scripts/ci.sh); locally a mismatch only warns — the pin is
    # authoritative for CI metrics, not a local build requirement.
    . "${SDK_DIR}/scripts/groundhog-cli-version.sh"

    command -v groundhog > /dev/null || {
        fail "groundhog CLI not found; install with: cargo install groundhog-cli --version \"=${GROUNDHOG_CLI_VERSION}\" --registry groundhog --locked"
        return 1
    }

    local installed
    installed=$(groundhog --version | awk '{print $2}')
    if [ "$installed" != "$GROUNDHOG_CLI_VERSION" ]; then
        warn "groundhog CLI ${installed} != pinned ${GROUNDHOG_CLI_VERSION} (CI uses the pin; continuing with the local CLI)"
    fi
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
        build-integration-rv32)
            itc_build_rv32_all
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
