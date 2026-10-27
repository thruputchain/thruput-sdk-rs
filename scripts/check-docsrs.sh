#!/usr/bin/env bash
# Builds each publishable workspace crate's documentation the way docs.rs
# does: `DOCS_RS` set, no network needed, and the crate's own
# [package.metadata.docs.rs] features, target and rustdoc args. The regular
# doc build (`dev.sh build-docs`) uses `--features=dev` across the workspace,
# which hides failures in any other configuration.
#
# Usage: check-docsrs.sh [package...]   (default: every publishable member)
set -euo pipefail

SDK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$SDK_DIR"

python3 - "$@" <<'PY'
import json
import os
import subprocess
import sys

meta = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--format-version", "1", "--no-deps"]))
publishable = {
    p["name"]: p for p in meta["packages"] if p["publish"] != []
}
names = sys.argv[1:] or sorted(publishable)

env = dict(
    os.environ,
    DOCS_RS="1",
    # Separate from the regular target dir: different features and cfgs
    # would otherwise invalidate it.
    CARGO_TARGET_DIR=os.path.join(meta["target_directory"], "docsrs"),
)
failed = []
for name in names:
    if name not in publishable:
        sys.exit(f"{name} is not a publishable workspace member")
    rs = (publishable[name].get("metadata") or {}).get("docs", {}).get("rs", {})
    cmd = ["cargo", "doc", "--no-deps", "-p", name]
    if rs.get("all-features"):
        cmd.append("--all-features")
    if rs.get("no-default-features"):
        cmd.append("--no-default-features")
    if rs.get("features"):
        cmd += ["--features", ",".join(rs["features"])]
    target = rs.get("default-target") or (rs.get("targets") or [None])[0]
    if target:
        cmd += ["--target", target]
    print(f"-- docs.rs build: {name} {rs or ''}", flush=True)
    result = subprocess.run(
        cmd, env=dict(env, RUSTDOCFLAGS=" ".join(rs.get("rustdoc-args", []))))
    if result.returncode != 0:
        failed.append(name)

if failed:
    sys.exit("docs.rs builds failed: " + ", ".join(failed))
print(f"OK: {len(names)} crates document in their docs.rs configuration")
PY
