#!/bin/bash

# These only work if this script is run in the SDK's top-level (workspace) directory.
SDK_DIR="$(realpath ".")"
TARGET_DIR="$(realpath "target")"

### BUILDING ###

cargo() {
    if [ -n "${CARGO_UNLOCK:-}" ]; then
        command cargo "$@"
    else
        command cargo --locked "$@"
    fi
}

RV32_TARGET="riscv32im-unknown-none-elf"
RV32_RELEASE="${TARGET_DIR}/${RV32_TARGET}/release"
# Where `groundhog build` writes the lowered artifacts:
# <stem>.{rv32a,rv32.json,spec.json,spec.xdr}.
LOWERED="${TARGET_DIR}/groundhog"

# rv32 contracts: the contract set (dirs with Cargo.toml minus the `disabled`
# marker, i.e. ls_contract_packages) narrowed to packages that declare a
# [[bin]]. Each contract hosts its own rv32 bin at src/main.rs; declaring it
# IS the rv32 enablement.
#
# The `disabled` marker is what keeps the negative fixtures
# (missing_contract_macro, dup_entrypoint_neg) and
# bigint-bench-contract out of this loop: they carry
# bins too, but must never be built here expecting success. The checks that
# own them build them by name.
#
# A package is recognized by an EXPLICIT `[[bin]]` in its manifest, not by
# cargo's target auto-discovery. That is deliberate — an auto-discovered bin
# carries no `required-features`, so host `cargo test` would select the
# `#![no_main]` rv32 bin and fail to link ("undefined symbol: main") — but it
# means a package relying on auto-discovery would drop out of the rv32 build,
# the dispatch/strings checks and the codesize metrics with no diagnostic at
# all. check_rv32_implicit_bin below turns that silence into a failure.
#
# Identical to ls_itc_rv32_packages below, and to the discovery in
# scripts/metrics/sdk_codesizes_rv32*.sh — keep the rules in sync.
ls_rv32_packages() {
    local contracts_dir
    contracts_dir="$1"; shift

    local package
    for package in $(ls_contract_packages "$contracts_dir"); do
        declares_bin "$contracts_dir/$package/Cargo.toml" || continue
        echo "$package"
    done
}

# True when a manifest declares a `[[bin]]` table. Leading whitespace is legal
# TOML, so match it rather than anchoring the header at column zero.
declares_bin() {
    grep -qE '^[[:space:]]*\[\[bin\]\]' "$1"
}

# A contract with a `src/main.rs` and no `[[bin]]` gets an auto-discovered bin
# target: buildable, but invisible to ls_rv32_packages (see its comment), and
# unguarded by `required-features` on the host. Both failures are silent, so
# fail here instead. `autobins = false` opts a package out of auto-discovery,
# which makes a bare `src/main.rs` inert and the omission correct.
check_rv32_implicit_bin() {
    local contracts_dir
    contracts_dir="$1"; shift

    info "Checking contracts declare their rv32 bin explicitly"

    local package manifest status
    status=0
    for package in $(ls_contract_packages "$contracts_dir"); do
        manifest="$contracts_dir/$package/Cargo.toml"
        [ -f "$contracts_dir/$package/src/main.rs" ] || continue
        declares_bin "$manifest" && continue
        grep -qE '^[[:space:]]*autobins[[:space:]]*=[[:space:]]*false' \
            "$manifest" && continue
        fail "$package has src/main.rs but no [[bin]]: cargo would auto-discover it (no required-features, so host builds link it) and the rv32 loop would skip it"
        status=1
    done
    return "$status"
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
        # The CLI itself passes --no-default-features and the package's
        # entrypoints (or rv32-entrypoints) feature — see the build_flags it
        # records in the .rv32.json sidecar. Callers may pass extra trailing
        # cargo args (e.g. `-F bottle-test`), which are additive on top.
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
        groundhog build -p "$package" -- \
            $locked -Zbuild-std=core,alloc "$@"
    )
}

check_rv32_dispatch() {
    local elf
    elf="$1"; shift

    bash "${SDK_DIR}/scripts/check-rv32-dispatch.sh" "$elf"
}

# Contracts built with the `debugging` feature (to match CrossExn wire format
# with the test runner) are expected to carry embedded path strings. Empty
# today: the rv32 profile builds with panic=immediate-abort, which strips the
# panic path — including `exceptions`, the one debugging-enabled fixture.
RV32_STRINGS_ALLOW_LIST=""

check_rv32_strings() {
    # Production rv32 builds (panic=immediate-abort, strip=symbols) must embed
    # no .rs file paths: they leak in from #[track_caller] and panic!()
    # locations.
    #
    # Takes package names as args; scans exactly those ELFs, not a directory
    # glob: the release dir is shared with root-workspace rv32 builds and
    # can hold stale ELFs from renamed or removed packages.
    info "Checking rv32 binaries for embedded path strings"

    local package elf rs_paths allowed
    for package in "$@"; do
        elf="${RV32_RELEASE}/${package//-/_}"
        [ -f "$elf" ] || {
            fail "missing rv32 ELF $elf (run build-examples-rv32 / build-integration-rv32 first)"
            return 1
        }
        rs_paths=$(strings "$elf" | grep -E '\.rs$' || true)
        if [ -n "$rs_paths" ]; then
            for allowed in $RV32_STRINGS_ALLOW_LIST; do
                if [ "$package" = "$allowed" ]; then
                    echo "SKIP: $(basename "$elf") (built with debugging)"
                    continue 2
                fi
            done
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
        # Bin names are NOT auto-converted by cargo: each contract must
        # declare `[[bin]] name` as the underscored package name (see
        # counter/Cargo.toml), which is what this path derivation assumes.
        elf="${RV32_RELEASE}/${package//-/_}"
        check_rv32_dispatch "$elf" || return 1
        check_rv32_mem_symbols "$elf" || return 1
    done
    check_rv32_implicit_bin "${SDK_DIR}/example-contracts" &&
    check_rv32_strings $(ls_rv32_packages "${SDK_DIR}/example-contracts") &&
    check_rv32_contract_spec_artifacts &&
    check_rv32_typedef_conflict_detected &&
    check_rv32_iface_declarer_conflict_detected &&
    check_rv32_event_signature_conflict_detected &&
    check_rv32_missing_entrypoints_detected &&
    check_rv32_duplicate_entrypoint_detected
}

# Negative spec fixture: typedef_id_collision points two declarations in one
# module at one spec id (`Decoy` takes `Colliding`'s via `name = "..."`) with
# different shapes, and keeps both reachable. The ELF links; the spec
# canonicalization the CLI runs during lowering must FAIL with a typedef
# conflict.
#
# Qualifying type names stopped same-simple-name types from colliding — see
# typedef_ids_coexist, which used to fail here and must now build — so this
# pins the half that must NOT relax: a genuine disagreement, two shapes
# claiming one id, is still ambiguous and must still be refused. The crate
# lives in negative-contracts/, a separate config chain from
# example-contracts/, which is for contracts that build and run.
check_rv32_typedef_conflict_detected() {
    info "CHECK-RV32-SPEC-CONFLICT-DETECTED"

    local out
    if out=$( (
        cd "${SDK_DIR}/negative-contracts" &&
        groundhog build -p typedef_id_collision
    ) 2>&1); then
        fail "Expected the rv32 build of typedef_id_collision to fail with a typedef conflict, but it succeeded"
        return 1
    fi
    # The message names the id, whose crate and version segments move with
    # every release, so match the part that identifies the conflict.
    echo "$out" | rg --quiet \
        "conflicting contract type definitions for .*::Colliding" || {
        fail "Expected a Colliding typedef conflict from typedef_id_collision, got: $out"
        return 1
    }
}

# Negative spec fixture: event_signature_collision declares one event ident in
# two of its own modules and emits both, so the two events have distinct spec
# ids and one signature — topic 0, and all a consumer has to tell them apart on
# the wire. The spec canonicalization the CLI runs during lowering must FAIL
# with an event signature conflict naming both (ConflictingEventSignature,
# covered by retained_events_sharing_a_signature_are_rejected).
check_rv32_event_signature_conflict_detected() {
    info "CHECK-RV32-EVENT-SIGNATURE-CONFLICT-DETECTED"

    local out
    if out=$( (
        cd "${SDK_DIR}/negative-contracts" &&
        groundhog build -p event_signature_collision
    ) 2>&1); then
        fail "Expected the rv32 build of event_signature_collision to fail with an event signature conflict, but it succeeded"
        return 1
    fi
    echo "$out" | rg --quiet \
        "events share an event signature: .*::ledger::Updated .* and .*::vault::Updated" || {
        fail "Expected an Updated event signature conflict from event_signature_collision, got: $out"
        return 1
    }
}

# Negative spec fixture: iface_declarer_collision implements two unrelated
# `Token` traits (ledger::Token, vault::Token). They share an interface key —
# name and major version, all the wire carries — so they share an interface
# id, while their distinct methods let the entrypoints link. The ELF links;
# spec canonicalization must FAIL naming both declarers, rather than publish
# one `Token` holding both method sets. (iface_conflict_impl, which
# implements one `Token` and only links the other, used to be refused here
# too; declarers tell its two candidates apart, so it now builds — see
# check_rv32_contract_spec_artifacts.)
check_rv32_iface_declarer_conflict_detected() {
    info "CHECK-RV32-IFACE-DECLARER-CONFLICT-DETECTED"

    local out
    if out=$( (
        cd "${SDK_DIR}/negative-contracts" &&
        groundhog build -p iface_declarer_collision
    ) 2>&1); then
        fail "Expected the rv32 build of iface_declarer_collision to fail with an interface declarer conflict, but it succeeded"
        return 1
    fi
    # Each declarer is followed by its crate@major, so match around those.
    echo "$out" | rg --quiet \
        "interface declared by more than one item: Token@.* is declared by both .*::ledger::Token .* and .*::vault::Token" || {
        fail "Expected a Token declarer conflict from iface_declarer_collision, got: $out"
        return 1
    }
}

# Negative fixture: missing_contract_macro forgot its #[contract] macro and
# does not hand-register an `Entrypoint` record either, so its dispatch table
# is empty and nothing can ever call it. Omitting the macro is legitimate on
# its own — raw_iface_dep and test_bytestring write their records by hand —
# so the guard is on the table being EMPTY, not on the macro being absent.
#
# check-rv32-dispatch.sh already enforces that for every contract in the build
# loop; this asserts it actually fires, on a fixture whose linked interface
# crate still populates the raw spec section (the shape that historically
# shipped silently as an uninvocable module with an empty spec).
#
# Built by name, unstripped: the bracket symbols the strong check reads are
# link-time constants that a stripped ELF drops. The crate carries a
# `disabled` marker so the rv32 build loop never builds it expecting
# success.
check_rv32_missing_entrypoints_detected() {
    info "CHECK-RV32-MISSING-ENTRYPOINTS-DETECTED"

    local locked="--locked"
    if [ -n "${CARGO_UNLOCK:-}" ]; then
        locked=""
    fi

    (
        cd "${SDK_DIR}/example-contracts" &&
        CARGO_PROFILE_RELEASE_STRIP=false \
        cargo build $locked -p missing_contract_macro -F entrypoints \
            --release --target "$RV32_TARGET" \
            -Zbuild-std=core,alloc \
            -Zbuild-std-features=std/optimize_for_size
    ) || {
        fail "Expected missing_contract_macro to LINK for rv32 (only its dispatch table should be empty)"
        return 1
    }

    local out
    if out=$(bash "${SDK_DIR}/scripts/check-rv32-dispatch.sh" \
        "${RV32_RELEASE}/missing_contract_macro" 2>&1); then
        fail "Expected the dispatch check to reject missing_contract_macro's empty table, but it passed: $out"
        return 1
    fi
    echo "$out" | rg -F --quiet "dispatch table is empty" || {
        fail "Expected an empty-dispatch-table error from missing_contract_macro, got: $out"
        return 1
    }
}

# Negative fixture: dup_entrypoint_neg has two colliding traits sharing a
# method name, so both impls produce the same entrypoint ID. Guard symbols
# carry the ID, so the build must FAIL with a duplicate-symbol error —
# replacing the previous silent behavior where two dispatch-table records
# shared an ID and the runtime's first-match-wins scan routed every call to
# one impl. The crate is workspace-excluded (its host build fails too, at
# codegen on the crate-scoped guards — see
# scripts/check-duplicate-entrypoint-detected.sh, which owns the host side)
# and carries a `disabled` marker so the rv32 build loop above never builds
# it expecting success.
check_rv32_duplicate_entrypoint_detected() {
    info "CHECK-RV32-DUP-ENTRYPOINT-DETECTED"

    local out
    if out=$( (
        cd "${SDK_DIR}/example-contracts/dup_entrypoint_neg" &&
        groundhog build -p dup_entrypoint_neg
    ) 2>&1); then
        fail "Expected the rv32 build of dup_entrypoint_neg to fail with a duplicate entrypoint symbol, but it succeeded"
        return 1
    fi
    echo "$out" | rg --quiet -e "already defined" -e "duplicate symbol" || {
        fail "Expected a duplicate-symbol error from dup_entrypoint_neg, got: $out"
        return 1
    }
}

# Same rule as ls_rv32_packages, against the integration_test contract dir:
# the contract set minus `disabled`, narrowed to packages declaring a
# [[bin]]. A contract with hand-written entrypoints and no bin is excluded by
# construction until someone gives it one. The metric scripts
# (scripts/metrics/sdk_codesizes_rv32*.sh) derive the same set — keep the
# rules in sync.
ls_itc_rv32_packages() {
    local contracts_dir="${SDK_DIR}/integration_test/contracts"
    local package
    for package in $(ls_contract_packages "$contracts_dir"); do
        declares_bin "$contracts_dir/$package/Cargo.toml" || continue
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
    check_rv32_implicit_bin "${SDK_DIR}/integration_test/contracts" &&
    check_rv32_strings $(ls_itc_rv32_packages)
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

assert_file_matches() {
    local path
    path="$1"; shift
    local pattern
    pattern="$1"; shift

    rg --quiet "$pattern" "$path" || {
        fail "Expected $path to match $pattern"
        return 1
    }
}

# Every user-defined thing the spec names — a type, an exception, an event —
# is named by a structured id (crate, major, module_path, declared), so these
# assert on the declared name rather than on a whole id: the crate and version
# fields move with every release and are not what any of these checks are
# about. Interface, method and argument names are NOT ids; assert on those
# with the plain helpers.
#
# The needle is the quoted `"declared":"Name"` field, so it is an exact match:
# `Payload` cannot match `AdoptedPayload`, and the `lacks` form still bites
# because the needle is exactly what a surviving id would contain.
assert_file_contains_id() {
    assert_file_contains "$1" "\"declared\":\"$2\""
}

assert_file_lacks_id() {
    assert_file_lacks "$1" "\"declared\":\"$2\""
}

# Spec content must not reach the deployed .rv32a memory image. Raw fragments
# live in the ELF's groundhogspecraw* section (where the CLI reads them); the
# lowerer must not copy them into the image, and rv32 has no embedded final spec
# (spec.json/.xdr are sidecars). Each needle is a spec-only name — the
# runtime never embeds them (dispatch uses hashed IDs, events hashed
# signatures) — asserted PRESENT in the ELF first so a vanished raw
# section (or renamed fixture type) cannot make the check pass vacuously.
assert_rv32a_lacks_spec_content() {
    local rv32a elf
    rv32a="$1"; shift
    elf="$1"; shift

    local needle
    for needle in "$@"; do
        rg -a -F --quiet "$needle" "$elf" || {
            fail "Expected $elf to contain spec needle $needle (raw spec section missing from the ELF?)"
            return 1
        }
        if rg -a -F --quiet "$needle" "$rv32a"; then
            fail "Spec content leaked into the lowered image $rv32a: found $needle"
            return 1
        fi
    done
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

    # The spec JSON is minified (no spaces), so needle matching is exact:
    # "name":"Orphan" (closing quote after n) cannot match
    # "name":"OrphanEvent" (E comes before the closing quote).
    # AdoptedPayload asserts positive typedef retention; OrphanPayload asserts
    # typedef shaking (both directions covered by a single fixture).
    local rv32_event_shaking_json rv32_typedef_shaking_json rv32_iface_conflict_json
    local iface_conflict_impl_json
    local free_fn_json imported_json multi_json spec_named_json genesis_json
    local erc20_json signer_callee_json signed_op_wallet_json coexist_json
    local name_reuse_json
    local phantom_method_json prerelease_json trait_sourced_json
    rv32_event_shaking_json="${groundhog_dir}/event_shaking_subset.spec.json"
    rv32_typedef_shaking_json="${groundhog_dir}/typedef_shaking_caller.spec.json"
    rv32_iface_conflict_json="${groundhog_dir}/iface_conflict_caller.spec.json"
    iface_conflict_impl_json="${groundhog_dir}/iface_conflict_impl.spec.json"
    free_fn_json="${groundhog_dir}/free_fn_unnamed.spec.json"
    imported_json="${groundhog_dir}/imported_error_callee.spec.json"
    multi_json="${groundhog_dir}/multi_imported_traits.spec.json"
    spec_named_json="${groundhog_dir}/spec_named_types.spec.json"
    genesis_json="${groundhog_dir}/genesis_deployer.spec.json"
    erc20_json="${groundhog_dir}/erc20_raw_storage.spec.json"
    signer_callee_json="${groundhog_dir}/signer_callee.spec.json"
    signed_op_wallet_json="${groundhog_dir}/signed_op_wallet.spec.json"
    phantom_method_json="${groundhog_dir}/phantom_method_impl.spec.json"
    prerelease_json="${groundhog_dir}/prerelease_version.spec.json"
    trait_sourced_json="${groundhog_dir}/trait_sourced_method_spec.spec.json"
    coexist_json="${groundhog_dir}/typedef_ids_coexist.spec.json"
    name_reuse_json="${groundhog_dir}/spec_name_reuse.spec.json"

    assert_file_contains "$free_fn_json" '"format":"gh_contract_spec_v1"' &&
    assert_file_contains "$free_fn_json" '"name":"_unnamed_interface"' &&
    assert_file_contains "$free_fn_json" '"version":"0.4.0"' &&

    assert_file_contains "$imported_json" '"name":"ImportedErrorCallee"' &&
    assert_file_contains_id "$imported_json" ImportedError &&

    assert_file_contains "$multi_json" '"name":"ExplicitNamedInterface"' &&
    assert_file_contains "$multi_json" '"name":"VersionedCallee"' &&
    assert_file_contains "$multi_json" '"version":"1.2.3"' &&

    assert_file_contains "$spec_named_json" '"type_defs":[' &&
    assert_file_contains "$spec_named_json" '"name":"SpecNamedTypesContract"' &&
    assert_file_contains_id "$spec_named_json" Payload &&
    assert_file_contains_id "$spec_named_json" Choice &&
    assert_file_contains_id "$spec_named_json" Envelope &&
    assert_file_contains "$spec_named_json" '"name":"echo_blob"' &&
    assert_file_contains "$spec_named_json" '"kind":"vec","element":{"kind":"u8"}' &&
    assert_file_contains "$spec_named_json" '"name":"echo_hash"' &&
    assert_file_contains "$spec_named_json" '"kind":"array","len":32,"element":{"kind":"u8"}' &&
    assert_file_contains "$spec_named_json" '"name":"echo_blob_envelope"' &&
    assert_file_contains_id "$spec_named_json" PayloadEvent &&
    assert_file_contains_id "$spec_named_json" AuditEvent &&
    assert_file_lacks_id "$spec_named_json" GhostEvent &&
    assert_file_lacks "$spec_named_json" '"name":"UnimplementedInterface"' &&

    # Phantom-method shaking: phantom_method_impl implements A's Ledger
    # (method ping) while linking B's colliding Ledger (method steal, payload
    # LootConfig) and carrying a same-crate collider (method phantom_local).
    # The contract must build, and its spec must advertise exactly the
    # methods with real entrypoints: ping only, with the phantom method's
    # payload typedef shaken too.
    assert_file_contains "$phantom_method_json" '"name":"Ledger"' &&
    assert_file_contains "$phantom_method_json" '"name":"ping"' &&
    assert_file_lacks "$phantom_method_json" '"name":"steal"' &&
    assert_file_lacks "$phantom_method_json" '"name":"phantom_local"' &&
    assert_file_lacks_id "$phantom_method_json" LootConfig &&

    # Method specs are sourced from the `#[contract]` trait, never from the
    # impl's own signatures. trait_sourced_method_spec's impl renames every
    # parameter `Pair` declares (`_factory`, `_numerator`, `_denominator` —
    # the unused-parameter convention); the published interface spec is a
    # contract's public API, so it must carry the trait's names, in the
    # trait's order. Trait and impl share a crate, so a regression also shows
    # up as a `conflicting method definitions` build failure — but only this
    # assertion catches an impl name that reaches the spec silently.
    #
    # Replaces the guard the deleted `swap` contract carried; the macro-layer
    # twin is `trait_impl_methods_come_from_trait_const_not_impl_signatures`.
    assert_file_contains "$trait_sourced_json" '"name":"initialize","args":[{"name":"factory"' &&
    assert_file_contains "$trait_sourced_json" '"name":"set_rate","args":[{"name":"numerator"' &&
    assert_file_contains "$trait_sourced_json" '"name":"denominator"' &&
    assert_file_lacks "$trait_sourced_json" '"_factory"' &&
    assert_file_lacks "$trait_sourced_json" '"_numerator"' &&
    assert_file_lacks "$trait_sourced_json" '"_denominator"' &&

    # Pre-release interface versions: the macro accepts them and the spec
    # pipeline must publish them verbatim (historically ingestion rejected
    # anything but bare MAJOR.MINOR.PATCH, so a pre-release contract
    # compiled and then failed at spec finalization).
    assert_file_contains "$prerelease_json" '"version":"0.9.0-rc.1"' &&

    # Raw contract (hand-written entrypoint records, no #[contract]) linking
    # an interface crate: the shaken candidates must leave no spec at all, not
    # a spec claiming `interfaces: []` just because a dependency carried
    # interface candidates. The rv32 pipeline writes no sidecar in that case.
    if [ -f "${groundhog_dir}/raw_iface_dep.spec.json" ]; then
        fail "Expected no spec.json sidecar for raw contract raw_iface_dep"
        return 1
    fi &&

    assert_file_contains "$erc20_json" '"events":[' &&
    assert_file_contains_id "$erc20_json" TransferEvent &&
    assert_file_contains_id "$erc20_json" AllowanceChangeEvent &&
    assert_file_contains "$erc20_json" '"name":"from","type":{"kind":"address"}' &&

    # No swap assertions: the swap* contracts are gone. The event coverage they
    # provided is carried by erc20_raw_storage above; the
    # trait-sourced-method-spec guard they carried has no replacement.

    assert_file_contains "$genesis_json" '"name":"GenesisDeployer"' &&
    assert_file_contains "$genesis_json" '"name":"deploy"' &&
    assert_file_contains "$genesis_json" '"name":"deploy_and_invoke"' &&

    # `Signed<T>` args/returns spec as signed_token nodes carrying the
    # preimage type (they consume no calldata bytes), never as u32 handles;
    # `Signed<Opaque>` is an erased (null) preimage.
    assert_file_matches "$signer_callee_json" '"name":"auth","type":\{"kind":"signed_token","preimage":\{"kind":"named","name":\{[^}]*"declared":"TransferAuth"\}\}\}' &&
    assert_file_contains_id "$signer_callee_json" TransferAuth &&
    assert_file_contains "$signed_op_wallet_json" '"kind":"signed_token","preimage":null' &&

    # Cross-crate event shaking: event_shaking_subset implements only the
    # `Adopted` trait from event_shaking_if; `Orphan` is never implemented.
    assert_file_contains "$rv32_event_shaking_json" '"name":"Adopted"' &&
    assert_file_contains_id "$rv32_event_shaking_json" AdoptedEvent &&
    assert_file_contains_id "$rv32_event_shaking_json" AdoptedPayload &&
    assert_file_lacks "$rv32_event_shaking_json" '"name":"Orphan"' &&
    assert_file_lacks_id "$rv32_event_shaking_json" OrphanEvent &&
    assert_file_lacks_id "$rv32_event_shaking_json" OrphanPayload &&

    # Typedef-collision shaking: the caller links two "versions" of a market
    # interface crate (typedef_shaking_if_v1/v2) purely to call them,
    # implementing neither trait. Each defines a differently shaped
    # `SharedConfig`; both defs co-travel into the raw section but are
    # unreachable, so the conflict must not fail the build (pre-fix this
    # build died with ConflictingTypeDef) and nothing from the market
    # interfaces may reach the final spec. Cross-version coverage: this
    # pipeline canonicalizes with the pinned CLI's embedded spec-tool, not
    # the workspace one.
    assert_file_contains "$rv32_typedef_shaking_json" '"name":"Caller"' &&
    assert_file_lacks_id "$rv32_typedef_shaking_json" SharedConfig &&
    assert_file_lacks "$rv32_typedef_shaking_json" '"name":"MarketV1"' &&
    assert_file_lacks "$rv32_typedef_shaking_json" '"name":"MarketV2"' &&

    # Interface-metadata conflict shaking: iface_conflict_caller links two
    # interface crates whose `Token` traits collide on the same interface key
    # (default name = trait ident, default version = workspace crate version)
    # with different emits metadata, implementing neither. Both candidates are
    # shaken, so the disagreement must not fail the build (pre-fix this build
    # died with ConflictingInterfaceEmits) and neither Token nor PingEvent may
    # reach the final spec.
    # Every collision qualified ids resolve, in one contract that builds. Each
    # pair below shares a bare ident — all the spec used to record — so each
    # was merged into one entry or rejected as a ConflictingTypeDef naming a
    # type the author never wrote, and this contract could not have worked
    # around it: two of the three pairs are its dependencies' names.
    #
    # Asserted per crate so both halves of a pair must be present: matching
    # `"declared":"Error"` alone would pass with only one of them.
    assert_file_matches "$name_reuse_json" '"crate":"dup_name_if_a",[^}]*"declared":"Error"' &&
    assert_file_matches "$name_reuse_json" '"crate":"dup_name_if_b",[^}]*"declared":"Error"' &&
    assert_file_matches "$name_reuse_json" '"crate":"dup_name_if_a",[^}]*"declared":"Updated"' &&
    assert_file_matches "$name_reuse_json" '"crate":"dup_name_if_b",[^}]*"declared":"Updated"' &&
    # Two modules of the contract's own crate reusing one ident.
    assert_file_contains "$name_reuse_json" '::ledger","declared":"Config"' &&
    assert_file_contains "$name_reuse_json" '::vault","declared":"Config"' &&

    # Two crates, one simple name, both defs in the pool: v1's is reachable
    # (the contract implements MarketV1) and v2's is not. Ids are what keep
    # them apart — before them this contract could not be built at all, and
    # the reference would have been ambiguous between the two shapes.
    assert_file_contains_id "$coexist_json" SharedConfig &&
    assert_file_contains "$coexist_json" '"crate":"typedef_shaking_if_v1"' &&
    assert_file_lacks "$coexist_json" '"crate":"typedef_shaking_if_v2"' &&

    assert_file_contains "$rv32_iface_conflict_json" '"name":"ConflictCaller"' &&
    assert_file_lacks "$rv32_iface_conflict_json" '"name":"Token"' &&
    assert_file_lacks_id "$rv32_iface_conflict_json" PingEvent &&

    # The anchored half of the same collision: iface_conflict_impl implements
    # A's Token and only links B's (to call a B token). Each candidate names
    # its declarer, so B's is shaken like any unimplemented interface and A's
    # metadata stands alone — A's PingEvent is kept. The declarer itself is
    # build-time only and must not reach the final spec.
    assert_file_contains "$iface_conflict_impl_json" '"name":"Token"' &&
    assert_file_contains_id "$iface_conflict_impl_json" PingEvent &&
    assert_file_lacks_id "$iface_conflict_impl_json" Token &&

    assert_rv32a_lacks_spec_content \
        "${groundhog_dir}/event_shaking_subset.rv32a" \
        "${RV32_RELEASE}/event_shaking_subset" \
        "AdoptedEvent" "OrphanPayload" &&
    assert_rv32a_lacks_spec_content \
        "${groundhog_dir}/typedef_shaking_caller.rv32a" \
        "${RV32_RELEASE}/typedef_shaking_caller" \
        "SharedConfig" "MarketV1" &&
    assert_rv32a_lacks_spec_content \
        "${groundhog_dir}/iface_conflict_caller.rv32a" \
        "${RV32_RELEASE}/iface_conflict_caller" \
        "PingEvent"
}

# Regression guard: the logging-test workspace unconditionally enables
# sdk/logging, which makes the codegen emit debug_log! calls in client
# stubs.  This catches use-after-move bugs for non-Copy by-value params
# (e.g. BlockHeight) that only surface under feature unification.
logging_test_build() {
    info "BUILD logging-test (rv32)"

    (
        cd "${SDK_DIR}/logging-test" &&
        cargo check -p htlc-logging --target "$RV32_TARGET" \
            -Zbuild-std=core,alloc
    )
}

# Regression guard: the raw spec fragment `#[derive(ContractTypeSpec)]` emits
# is behind a `cfg(target_arch = "riscv32")`, so a host build never compiles
# it. Every other consumer reaches that code through `::sdk`, which the
# example/integration rv32 builds cover -- but nothing else builds a crate
# that uses `#[contract_type_spec(crate = "...")]`, and its whole point is
# resolving those paths through a shim instead. Without this, a regression in
# the overridden guest path surfaces only when a downstream crate below `sdk`
# builds for a guest target.
macros_crate_override_guest_build() {
    info "BUILD groundhog-macros crate-override test (rv32)"

    # A dedicated no_std package rather than `-p groundhog-macros --tests`:
    # that would build the proc-macro crate's own unit tests for rv32, and
    # its deps (proc-macro2 &co) need std.
    cargo check -p macros-crate-override-guest \
        --target "$RV32_TARGET" -Zbuild-std=core,alloc
}

### LINT ###

lint_contracts() {
    local workspace_dir
    workspace_dir="$1"; shift
    local packages_dir
    packages_dir="${1:-$workspace_dir}"; shift 2>/dev/null || true

    [ -d "$packages_dir" ] || {
        fail "Contract directory not found: $packages_dir"
        return 1
    }

    local contract_packages
    contract_packages=$(ls_contract_packages "$packages_dir") || return 1
    [ -n "$contract_packages" ] || {
        fail "No enabled contract packages found: $packages_dir (initialize submodules)"
        return 1
    }

    local package
    for package in $contract_packages ; do
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

        # Guest lint: the workspace .cargo/config.toml supplies the rv32
        # rustflags; build-std goes on the command line (see build_rv32).
        cargo clippy -p "$package" \
                -F entrypoints \
                --release \
                --target "$RV32_TARGET" \
                -Zbuild-std=core,alloc \
                -- -Dwarnings &&

        cargo clippy -F dev -p "$package"
    )
}

### TESTING ###

test_package() {
    cargo test &&
    # u256's self-dev-dep unifies std+fmt into every test build, so the
    # no-feature guest config is only ever compiled, never tested; check it
    # explicitly so cfg-gating mistakes fail here and not in a contract build.
    cargo check -p groundhog-u256 --no-default-features &&
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
    # Note: Some of these tests require their associated rv32 artifacts to be
    # built first (scripts/dev.sh build-examples-rv32).

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

BOTTLE_TEST="${TARGET_DIR}/bottle-test"

# Builds the contract twice for rv32 and runs the pair through the harness:
#
#   <stem>.target.rv32a   the contract under measurement (entrypoints only)
#   <stem>.tester.rv32a   the same contract plus the BottleTest entrypoints
#                         that drive it (the package's `bottle-test` feature)
#
# `groundhog build` writes both to the same target/groundhog/<stem>.rv32a, so
# each is copied aside before the next build overwrites it — the same staging
# scripts/bigint-bench.sh does for its per-variant artifacts.
#
# `package` is the contract package (e.g. erc20); its `[[bin]]` name is the
# underscored package name, which is the artifact stem.
bottle_test_contract() {
    local workspace_dir package
    workspace_dir="$1"; shift
    package="$1"; shift

    info "BOTTLE-TEST $package"

    require_groundhog_cli || return 1

    local stem="${package//-/_}"

    mkdir -p "$BOTTLE_TEST" &&

    build_rv32 "$workspace_dir" "$package" "$@" &&
    cp "${LOWERED}/${stem}.rv32a" "${BOTTLE_TEST}/${stem}.target.rv32a" &&

    build_rv32 "$workspace_dir" "$package" -F bottle-test "$@" &&
    cp "${LOWERED}/${stem}.rv32a" "${BOTTLE_TEST}/${stem}.tester.rv32a" &&

    cargo run -p bottle-test --profile dev "$stem"
}

has_bottle_test() {
    local contract cargo_toml
    contract="$1"; shift
    cargo_toml="$contract/Cargo.toml"

    grep -s "^bottle-test = " "$cargo_toml" >/dev/null
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

# The bottle-tested example contracts are the rv32 wrapper packages that
# declare a `bottle-test` feature (which forwards to the library's).
eg_bottle_test_all() {
    local package
    for package in $(ls_rv32_packages "${SDK_DIR}/example-contracts"); do
        has_bottle_test "example-contracts/$package" || continue
        [ ! -f "example-contracts/$package/disabled-bottle" ] || continue

        bottle_test_contract "${SDK_DIR}/example-contracts" "$package" ||
            return 1
    done
}

# integration_test contracts host their own rv32 bin (see
# ls_itc_rv32_packages), so the bottle-test feature lives on the contract
# package itself rather than on a sibling wrapper.
itc_bottle_test_all() {
    local package
    for package in $(ls_itc_rv32_packages); do
        has_bottle_test "integration_test/contracts/$package" || continue
        [ ! -f "integration_test/contracts/$package/disabled-bottle" ] ||
            continue

        bottle_test_contract "${SDK_DIR}/integration_test" "$package" ||
            return 1
    done
}

### BIG-INTEGER BENCHMARK ###

# rv32 codesize + gas for 256-bit arithmetic, per operation, per implementation.
#
# Deliberately not part of `ci`: it builds the benchmark contract once per
# (backend, operation) pair -- 34 rv32 builds with LTO today -- which is minutes
# of work for a number that only moves when the arithmetic does. Run it on
# demand, or from scripts/metrics/sdk_bigint_bench_rv32.sh.
bigint_bench() {
    require_groundhog_cli || return 1

    bash "${SDK_DIR}/scripts/bigint-bench.sh" "$@"
}

# The benchmark's host-side differential test: both backends must agree on every
# operation in the sweep. Cheap (no rv32, no node), and the check that keeps a
# codesize or gas comparison honest -- a mis-mapped operation would otherwise
# read as a performance win. See bigint-bench-contract/tests/differential.rs.
#
# Needs its own entry because the package carries a `disabled` marker, so
# eg_test_all skips it along with the rv32 wrapper glob.
bigint_bench_test() {
    info "TEST bigint-bench-contract (backend differential)"
    (
        cd "${SDK_DIR}/example-contracts" &&
        cargo test -p bigint-bench-contract
    )
}

build_docs() {
    RUSTDOCFLAGS="--deny warnings --cfg docsrs" \
        cargo doc --features=dev "$@"
}

# Build the sdk itself for the guest target.
#
# The host test suite never compiles for rv32, so a guest-only break -- a
# `cfg` slip, or a bound that only the host can satisfy -- reaches CI only
# through the contract sweeps much later in this run. This is the same
# compile, seconds in, with the error pointing at `sdk` rather than at
# whichever example contract happened to instantiate it first.
sdk_rv32_build() {
    info "BUILD(rv32) sdk"
    cargo build -p sdk --release --target "$RV32_TARGET"
}

skip() {
    local var="SKIP_$1"
    [ "${!var:-}" = "1" ]
}

check_all() {
    { skip SDK_RV32 || sdk_rv32_build; } &&
    { skip DOCS || build_docs; } &&
    { skip SDK_TESTS || test_package; } &&

    { skip EXAMPLES || {
        { skip RV32_EXAMPLES || eg_build_rv32_all; } &&
        { skip TEST_EXAMPLES || eg_test_all; } &&
        { skip BOTTLE_EXAMPLES || eg_bottle_test_all; } &&
        { skip DOMAIN_COLLISION || bash "${SDK_DIR}/scripts/check-domain-collision-detected.sh"; } &&
        { skip DUP_ENTRYPOINT || bash "${SDK_DIR}/scripts/check-duplicate-entrypoint-detected.sh"; } &&
        { skip REGISTRY_STRIPPED || bash "${SDK_DIR}/scripts/check-storage-registry-stripped.sh"; }
    }; } &&

    { skip INTEGRATION || {
        { skip RV32_INTEGRATION || itc_build_rv32_all; } &&
        itc_test_all &&
        itc_bottle_test_all
    }; } &&

    { skip LOGGING || logging_test_build; } &&

    { skip MACROS_GUEST || macros_crate_override_guest_build; } &&

    # now that Cargo.lock files have been updated:
    cargo run -p version-check &&

    info "All tests succeeded." ||
    fail "Some tests failed"
}

### Other dev utils

# Guest dependency tree / macro expansion. Both run against the rv32 target
# so what they show is what a contract actually compiles; the workspace
# .cargo/config.toml supplies the rustflags and build-std goes on the command
# line (see build_rv32).
tree_contract() {
    local contract
    contract=$(realpath "$1"); shift

    (
        cd "$contract" &&
        cargo tree \
            --target "$RV32_TARGET" \
            --edges no-proc-macro,no-build,no-dev \
            -Z build-std=core,alloc \
            -Z build-std-features=std/optimize_for_size \
            "$@"
    )
}

expand_contract() {
    local contract
    contract=$(realpath "$1"); shift

    (
        cd "$contract" &&
        cargo expand \
            --features "entrypoints" \
            --target "$RV32_TARGET" \
            --color=always \
        #
    )
}

# The deployed rv32 artifact's entrypoint table, as recorded by the lowerer.
contract_entrypoints() {
    local package stem
    package=$(basename "$1"); shift
    stem="${package//-/_}"

    ls -l "${LOWERED}/${stem}.rv32a" &&
    cat "${LOWERED}/${stem}.rv32.json"
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
    lint_contracts "integration_test" "integration_test/contracts"
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
        build|build-rv32)
            dir=$(dirname "$1")
            pkg=$(basename "$1")

            build_rv32 "$dir" "$pkg"
            ;;
        build-examples-rv32)
            eg_build_rv32_all
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
                test_package
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
        run-bottle-tests)
            eg_bottle_test_all &&
                itc_bottle_test_all
            ;;
        bigint-bench)
            bigint_bench "$@"
            ;;
        bigint-bench-test)
            bigint_bench_test
            ;;
        entrypoints)
            contract_entrypoints "$@"
            ;;
        strings)
            check_rv32_strings "$@"
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
