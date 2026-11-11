#!/usr/bin/env python3
"""Copy dependency overrides into a standalone source-checkout smoke test."""

import json
from pathlib import Path
import sys
import tomllib

source, destination = map(Path, sys.argv[1:])
patches = tomllib.loads(source.read_text()).get("patch", {})
existing = tomllib.loads(destination.read_text()) if destination.exists() else {}
lines = []
for registry, packages in patches.items():
    for name, spec in packages.items():
        spec = dict(spec)
        if "path" in spec:
            # Cargo config paths are relative to the directory above .cargo.
            spec["path"] = str((source.parent.parent / spec["path"]).resolve())
        previous = existing.get("patch", {}).get(registry, {}).get(name)
        if previous is not None:
            if previous != spec:
                raise SystemExit(f"conflicting consumer patch: {registry}/{name}")
            continue
        lines.append(f"\n[patch.{json.dumps(registry)}.{json.dumps(name)}]\n")
        lines.extend(
            f"{json.dumps(key)} = {json.dumps(value, ensure_ascii=False)}\n"
            for key, value in spec.items()
        )

if lines:
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("a") as config:
        config.writelines(lines)
