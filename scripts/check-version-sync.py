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
    runtime = plugin / "runtime" / "calendar-adapter"
    source_path = runtime / "calendar-adapter.swift"
    plist_path = runtime / "Info.plist"
    if not source_path.exists() and not plist_path.exists():
        return True
    if not source_path.exists() or not plist_path.exists():
        print(
            f"[release:adapter-version:error] Incomplete Swift adapter metadata: {plugin.name}",
            file=sys.stderr,
        )
        return False

    source = source_path.read_text(encoding="utf-8")
    match = re.search(r'private let calendarAdapterVersion = "([^"]+)"', source)
    if match is None:
        print(
            f"[release:adapter-version:error] calendarAdapterVersion is missing: {plugin.name}",
            file=sys.stderr,
        )
        return False

    with plist_path.open("rb") as handle:
        plist_version = plistlib.load(handle).get("CFBundleShortVersionString")
    source_version = match.group(1)
    if source_version != plist_version:
        print(
            f"[release:adapter-version:error] Calendar adapter version mismatch: {plugin.name}",
            file=sys.stderr,
        )
        print(f"  {source_path.relative_to(plugin)}: {source_version}", file=sys.stderr)
        print(f"  {plist_path.relative_to(plugin)}: {plist_version}", file=sys.stderr)
        return False

    print(
        f"[release:adapter-version:success] plugin={plugin.name} "
        f"calendar-adapter-version={source_version}"
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
        "application": read_cargo_version(plugin / "crates" / "sherpa" / "Cargo.toml"),
        "contextDomain": read_cargo_version(
            plugin / "crates" / "context-engine" / "Cargo.toml"
        ),
        "plannerMetadata": read_cargo_version(
            plugin / "crates" / "planner-metadata" / "Cargo.toml"
        ),
    }

    swift_source = (
        plugin / "runtime" / "calendar-adapter" / "calendar-adapter.swift"
    ).read_text(
        encoding="utf-8"
    )
    swift_match = re.search(
        r'private let calendarAdapterVersion = "([^"]+)"', swift_source
    )
    if swift_match is None:
        print(
            "[release:runtime-version:error] Sherpa calendarAdapterVersion is missing",
            file=sys.stderr,
        )
        return False
    actual["calendarAdapter"] = swift_match.group(1)

    failures: list[str] = []
    for name, actual_version in actual.items():
        declared_version = declared.get(name)
        if declared_version != actual_version:
            failures.append(
                f"{name}: declared={declared_version} actual={actual_version}"
            )

    reminders_adapter = declared.get("remindersAdapter")
    if not isinstance(reminders_adapter, dict):
        failures.append("remindersAdapter: runtime declaration is missing")
    else:
        installer = (plugin / "scripts" / "install-runtime.sh").read_text(
            encoding="utf-8"
        )
        doctor = (plugin / "scripts" / "doctor.sh").read_text(encoding="utf-8")
        expected_installer_values = {
            "application": f'SHERPA_VERSION="{declared.get("application")}"',
            "calendarAdapter": f'CALENDAR_ADAPTER_VERSION="{declared.get("calendarAdapter")}"',
            "remindersAdapter.version": (
                f'REMINDERS_ADAPTER_VERSION="{reminders_adapter.get("version")}"'
            ),
            "version": f'REMCTL_TAG="v{reminders_adapter.get("version")}"',
            "gitCommit": f'REMCTL_COMMIT="{reminders_adapter.get("gitCommit")}"',
            "source": f'REMCTL_SOURCE="{reminders_adapter.get("source")}"',
        }
        for field, shell_value in expected_installer_values.items():
            if shell_value not in installer:
                failures.append(f"{field}: installer pin differs")

        expected_doctor_values = {
            "application": f'SHERPA_VERSION="{declared.get("application")}"',
            "calendarAdapter": f'CALENDAR_ADAPTER_VERSION="{declared.get("calendarAdapter")}"',
            "remindersAdapter.version": (
                f'REMINDERS_ADAPTER_VERSION="{reminders_adapter.get("version")}"'
            ),
            "remindersAdapter.gitCommit": (
                f'REMCTL_COMMIT="{reminders_adapter.get("gitCommit")}"'
            ),
            "remindersAdapter.source": (
                f'REMCTL_SOURCE="{reminders_adapter.get("source")}"'
            ),
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
        f"plugin={plugin.name} application={actual['application']} "
        f"context-domain={actual['contextDomain']} "
        f"planner-metadata={actual['plannerMetadata']} "
        f"calendar-adapter={actual['calendarAdapter']} "
        f"reminders-adapter={reminders_adapter['version']}"
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
