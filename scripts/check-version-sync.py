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
    versions = {
        ".codex-plugin/plugin.json": read_json(
            plugin / ".codex-plugin" / "plugin.json"
        )["version"],
        ".claude-plugin/plugin.json": read_json(
            plugin / ".claude-plugin" / "plugin.json"
        )["version"],
    }

    cargo_path = plugin / "Cargo.toml"
    if cargo_path.is_file():
        with cargo_path.open("rb") as handle:
            cargo_document = tomllib.load(handle)
        package = cargo_document.get("package")
        if isinstance(package, dict) and isinstance(package.get("version"), str):
            versions["Cargo.toml"] = package["version"]

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
        f"[release:version:success] plugin={plugin.name} "
        f"base-version={next(iter(base_versions.values()))}"
    )
    return True


def check_swift_adapter(plugin: Path) -> bool:
    direct_runtime = plugin / "runtime"
    nested_runtime = direct_runtime / "calctl"
    if (nested_runtime / "calctl.swift").exists() or (nested_runtime / "Info.plist").exists():
        source_path = nested_runtime / "calctl.swift"
        plist_path = nested_runtime / "Info.plist"
    else:
        source_path = direct_runtime / "calctl.swift"
        plist_path = direct_runtime / "Info.plist"
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
        print(f"  {source_path.relative_to(plugin)}: {source_version}", file=sys.stderr)
        print(f"  {plist_path.relative_to(plugin)}: {plist_version}", file=sys.stderr)
        return False

    print(
        f"[release:adapter-version:success] plugin={plugin.name} "
        f"calctl-version={source_version}"
    )
    return True


def read_cargo_version(path: Path) -> str:
    with path.open("rb") as handle:
        document = tomllib.load(handle)
    return document["package"]["version"]


def check_sherpa_runtime_versions(plugin: Path) -> bool:
    versions_path = plugin / "runtime-versions.json"
    if not versions_path.is_file():
        return True

    declared = read_json(versions_path)
    actual = {
        "plugin": read_json(plugin / ".claude-plugin" / "plugin.json")["version"],
        "calmeta": read_cargo_version(plugin / "crates" / "calmeta" / "Cargo.toml"),
        "msgpipe": read_cargo_version(plugin / "crates" / "msgpipe" / "Cargo.toml"),
    }

    swift_source = (plugin / "runtime" / "calctl" / "calctl.swift").read_text(
        encoding="utf-8"
    )
    swift_match = re.search(r'private let calctlVersion = "([^"]+)"', swift_source)
    if swift_match is None:
        print("[release:runtime-version:error] Sherpa calctlVersion is missing", file=sys.stderr)
        return False
    actual["calctl"] = swift_match.group(1)

    failures: list[str] = []
    for name, actual_version in actual.items():
        declared_version = declared.get(name)
        if declared_version != actual_version:
            failures.append(
                f"{name}: declared={declared_version} actual={actual_version}"
            )

    remctl = declared.get("remctl")
    if not isinstance(remctl, dict):
        failures.append("remctl: runtime declaration is missing")
    else:
        installer = (plugin / "scripts" / "install-runtime.sh").read_text(
            encoding="utf-8"
        )
        doctor = (plugin / "scripts" / "doctor.sh").read_text(encoding="utf-8")
        expected_installer_values = {
            "calmeta": f'CALMETA_VERSION="{declared.get("calmeta")}"',
            "calctl": f'CALCTL_VERSION="{declared.get("calctl")}"',
            "msgpipe": f'MSGPIPE_VERSION="{declared.get("msgpipe")}"',
            "version": f'REMCTL_TAG="v{remctl.get("version")}"',
            "gitCommit": f'REMCTL_COMMIT="{remctl.get("gitCommit")}"',
            "source": f'REMCTL_SOURCE="{remctl.get("source")}"',
        }
        for field, shell_value in expected_installer_values.items():
            if shell_value not in installer:
                failures.append(f"{field}: installer pin differs")

        expected_doctor_values = {
            "calmeta": f'CALMETA_VERSION="{declared.get("calmeta")}"',
            "calctl": f'CALCTL_VERSION="{declared.get("calctl")}"',
            "msgpipe": f'MSGPIPE_VERSION="{declared.get("msgpipe")}"',
            "remctl.version": f'REMCTL_VERSION="{remctl.get("version")}"',
            "remctl.gitCommit": f'REMCTL_COMMIT="{remctl.get("gitCommit")}"',
            "remctl.source": f'REMCTL_SOURCE="{remctl.get("source")}"',
        }
        for field, shell_value in expected_doctor_values.items():
            if shell_value not in doctor:
                failures.append(f"{field}: doctor pin differs")

    if failures:
        print("[release:runtime-version:error] Sherpa runtime mismatch", file=sys.stderr)
        for failure in failures:
            print(f"  {failure}", file=sys.stderr)
        return False

    print(
        "[release:runtime-version:success] "
        f"plugin={plugin.name} calmeta={actual['calmeta']} calctl={actual['calctl']} "
        f"msgpipe={actual['msgpipe']} remctl={remctl['version']}"
    )
    return True


def main() -> int:
    plugins = sorted(
        path.parent.parent
        for path in (ROOT / "plugins").glob("*/.codex-plugin/plugin.json")
    )
    if not plugins:
        print("[release:version:error] No Rust plugins found", file=sys.stderr)
        return 1
    results = [
        check_plugin(plugin)
        and check_swift_adapter(plugin)
        and check_sherpa_runtime_versions(plugin)
        for plugin in plugins
    ]
    return 0 if all(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
