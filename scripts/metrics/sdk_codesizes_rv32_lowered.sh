#!/bin/bash
set -euo pipefail

# Per-contract lowered rv32 artifact metrics: the true on-chain numbers,
# read from the `thruput build` metadata sidecars
# (<name>.rv32.json in target/thruput/). sdk_codesizes_rv32 keeps the
# per-section detail of the unstripped ELF; this metric records what the
# lowerer actually emits.
#
# Output format (one line per contract, append-only metric history —
# do not reorder or rename fields):
#   <name> code=<B> initial_memory=<B> memory=<B> artifact=<B>
#
#   code           = lowered code image bytes         (sidecar code_bytes)
#   initial_memory = stored initial data image bytes  (initial_memory_bytes)
#   memory         = total VM memory reservation      (memory_bytes)
#   artifact       = full .rv32a artifact size        (artifact_bytes)

if [ $# -lt 1 ] || [ $# -gt 3 ]; then
  echo "Usage: $0 <Thruput artifact dir> [example contracts dir] [itc contracts dir]" >&2
  exit 1
fi

ARTIFACT_DIR=$1
# The metric input set is derived from the source tree (`cargo gnaw packages
# --rv32`, run from the repository root), NOT from globbing the artifact
# dir: target/thruput/ is shared across both contract workspaces and can
# hold stale artifacts from renamed or removed contracts. None of those may
# emit a row into the append-only history, and a missing sidecar for a
# current contract must fail loudly.
CONTRACTS_DIR=${2:-example-contracts}
ITC_CONTRACTS_DIR=${3:-integration_test/contracts}

json_int() {
    # Integer field $2 from sidecar $1. The sidecar is generated with
    # sorted keys and 2-space pretty-printing by the pinned Thruput CLI,
    # so line-based extraction is stable; fail loudly rather than emit a
    # missing/empty value into append-only history.
    local val
    val=$(sed -n "s/^ *\"$2\": \([0-9][0-9]*\),*$/\1/p" "$1")
    [ -n "$val" ] || {
        echo "field $2 missing from $1" >&2
        exit 1
    }
    echo "$val"
}

emit_row() {
    local name=$1
    local sidecar="$ARTIFACT_DIR/$name.rv32.json"
    if [ ! -f "$sidecar" ]; then
        echo "Expected lowered rv32 sidecar missing: $sidecar (build with cargo gnaw run metrics-inputs)" >&2
        exit 1
    fi
    # Plain assignments (not $() inline in echo args): set -e aborts on a
    # failed substitution here, so a missing field can never emit a
    # partial line into append-only history.
    local code initial_memory memory artifact
    code=$(json_int "$sidecar" code_bytes)
    initial_memory=$(json_int "$sidecar" initial_memory_bytes)
    memory=$(json_int "$sidecar" memory_bytes)
    artifact=$(json_int "$sidecar" artifact_bytes)
    echo "$name code=$code initial_memory=$initial_memory memory=$memory artifact=$artifact"
}

# The rv32 contracts the fixture and integration tasks build: gnaw's
# discovery, from the source tree. Name = underscored package name.
names=$(cargo gnaw packages --rv32 "$CONTRACTS_DIR" "$ITC_CONTRACTS_DIR")
names=${names//-/_}

if [ -z "$names" ]; then
    echo "No rv32 contracts found ($CONTRACTS_DIR, $ITC_CONTRACTS_DIR)" >&2
    exit 1
fi

# target/thruput/ is one flat namespace shared by both workspaces: a
# duplicate snake name would silently attribute one contract's numbers to
# the other, so fail loudly instead.
seen=" "
for name in $names; do
    case "$seen" in
        *" $name "*)
            echo "duplicate rv32 contract name: $name" >&2
            exit 1
            ;;
    esac
    seen="$seen$name "
    emit_row "$name"
done
