#!/bin/bash

# Compile the Cargo-only guide as a standalone consumer, with no CLI and no
# inherited SDK workspace configuration. Keep the guide itself as the fixture.
set -euo pipefail

SDK_DIR="$(cd "$(dirname "$0")/.." && pwd)"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/thruput-cargo-example.XXXXXX")"
trap 'rm -rf "$WORK_DIR"' EXIT

python3 - "$SDK_DIR" "$WORK_DIR" <<'PY'
import json
from pathlib import Path
import re
import shutil
import sys

sdk, work = map(Path, sys.argv[1:])
guide = (sdk / 'crates/sdk/doc/cargo-only.md').read_text()
files = dict(re.findall(r'^### `([^`]+)`\n\n```(?:toml|rust)\n(.*?)\n```', guide, re.M | re.S))
expected = {'Cargo.toml', '.cargo/config.toml', 'src/lib.rs', 'src/main.rs', 'tests/greeting.rs'}
if files.keys() != expected:
    raise SystemExit(f'Cargo guide file blocks changed: {sorted(files)}')
for name, content in files.items():
    if name == 'Cargo.toml':
        content = content.replace('"../thruput-sdk-rs/crates/sdk"', json.dumps(str(sdk / 'crates/sdk')))
    path = work / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content + '\n')
shutil.copyfile(sdk / 'rust-toolchain.toml', work / 'rust-toolchain.toml')
PY

cd "$WORK_DIR"
cargo test
cargo --config "$SDK_DIR/example-contracts/.cargo/config.toml" build \
    --release --target riscv32im-unknown-none-elf \
    --no-default-features --features entrypoints --bin hello_contract \
    -Zbuild-std=core,alloc --target-dir target
