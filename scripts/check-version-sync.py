#!/usr/bin/env python3

import json
import plistlib
import re
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def read_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def check_plugin(plugin: Path) -> bool:
    with (plugin / "Cargo.toml").open("rb") as handle:
        cargo_version = tomllib.load(handle)["package"]["version"]

    versions = {
        "Cargo.toml": cargo_version,
        ".codex-plugin/plugin.json": read_json(
            plugin / ".codex-plugin" / "plugin.json"
        )["version"],
        ".claude-plugin/plugin.json": read_json(
            plugin / ".claude-plugin" / "plugin.json"
        )["version"],
    }

    base_versions = {source: version.split("+", 1)[0] for source, version in versions.items()}

    if len(set(base_versions.values())) != 1:
        print(
            f"[release:version:error] Version mismatch: {plugin.name}",
            file=sys.stderr,
        )
        for source, version in versions.items():
            print(f"  {source}: {version}", file=sys.stderr)
        return False

    print(
        f"[release:version:success] plugin={plugin.name} base-version={cargo_version}"
    )
    return True


def check_swift_adapter(plugin: Path) -> bool:
    source_path = plugin / "runtime" / "calctl.swift"
    plist_path = plugin / "runtime" / "Info.plist"
    if not source_path.exists() and not plist_path.exists():
        return True
    if not source_path.exists() or not plist_path.exists():
        print(
            f"[release:adapter-version:error] Incomplete Swift adapter metadata: {plugin.name}",
            file=sys.stderr,
        )
        return False

    source = source_path.read_text(encoding="utf-8")
    match = re.search(r'private let calctlVersion = "([^"]+)"', source)
    if match is None:
        print(
            f"[release:adapter-version:error] calctlVersion is missing: {plugin.name}",
            file=sys.stderr,
        )
        return False

    with plist_path.open("rb") as handle:
        plist_version = plistlib.load(handle).get("CFBundleShortVersionString")
    source_version = match.group(1)
    if source_version != plist_version:
        print(
            f"[release:adapter-version:error] calctl version mismatch: {plugin.name}",
            file=sys.stderr,
        )
        print(f"  runtime/calctl.swift: {source_version}", file=sys.stderr)
        print(f"  runtime/Info.plist: {plist_version}", file=sys.stderr)
        return False

    print(
        f"[release:adapter-version:success] plugin={plugin.name} "
        f"calctl-version={source_version}"
    )
    return True


def main() -> int:
    plugins = sorted(
        path.parent for path in (ROOT / "plugins").glob("*/Cargo.toml")
    )
    if not plugins:
        print("[release:version:error] No Rust plugins found", file=sys.stderr)
        return 1
    results = [
        check_plugin(plugin) and check_swift_adapter(plugin) for plugin in plugins
    ]
    return 0 if all(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
