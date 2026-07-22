#!/usr/bin/env python3

import json
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
PLUGIN = ROOT / "plugins" / "message-pipeline"


def read_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def main() -> int:
    with (PLUGIN / "Cargo.toml").open("rb") as handle:
        cargo_version = tomllib.load(handle)["package"]["version"]

    versions = {
        "Cargo.toml": cargo_version,
        ".codex-plugin/plugin.json": read_json(
            PLUGIN / ".codex-plugin" / "plugin.json"
        )["version"],
        ".claude-plugin/plugin.json": read_json(
            PLUGIN / ".claude-plugin" / "plugin.json"
        )["version"],
    }

    base_versions = {source: version.split("+", 1)[0] for source, version in versions.items()}

    if len(set(base_versions.values())) != 1:
        print("[release:version:error] Version mismatch", file=sys.stderr)
        for source, version in versions.items():
            print(f"  {source}: {version}", file=sys.stderr)
        return 1

    print(f"[release:version:success] base-version={cargo_version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
