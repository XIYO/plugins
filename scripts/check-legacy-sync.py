#!/usr/bin/env python3

from __future__ import annotations

import hashlib
import os
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
VALID_LOG_LEVELS = {"debug": 10, "info": 20, "warn": 30, "error": 40}
LOG_LEVEL = VALID_LOG_LEVELS.get(os.getenv("LOG_LEVEL", "warn").lower(), 30)

FILE_MAPPINGS = (
    ("plugins/sherpa/crates/calmeta/Cargo.toml", "plugins/apple-calendar/Cargo.toml"),
    ("plugins/sherpa/crates/msgpipe/Cargo.toml", "plugins/message-pipeline/Cargo.toml"),
)

DIRECTORY_MAPPINGS = (
    ("plugins/sherpa/crates/calmeta/src", "plugins/apple-calendar/src"),
    ("plugins/sherpa/runtime/calctl", "plugins/apple-calendar/runtime"),
    ("plugins/sherpa/crates/msgpipe/src", "plugins/message-pipeline/src"),
    ("plugins/sherpa/crates/msgpipe/tests", "plugins/message-pipeline/tests"),
)


def log(level: str, scope: str, message: str) -> None:
    if VALID_LOG_LEVELS[level] >= LOG_LEVEL:
        print(f"[{scope}:{level}] {message}", file=sys.stderr)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def compare_file(canonical_relative: str, compatibility_relative: str) -> list[str]:
    canonical = ROOT / canonical_relative
    compatibility = ROOT / compatibility_relative
    if not canonical.is_file():
        return [f"missing Sherpa canonical file: {canonical_relative}"]
    if not compatibility.is_file():
        return [f"missing compatibility copy: {compatibility_relative}"]
    if digest(canonical) != digest(compatibility):
        return [f"content differs: {canonical_relative} != {compatibility_relative}"]
    return []


def compare_directory(canonical_relative: str, compatibility_relative: str) -> list[str]:
    canonical = ROOT / canonical_relative
    compatibility = ROOT / compatibility_relative
    if not canonical.is_dir():
        return [f"missing Sherpa canonical directory: {canonical_relative}"]
    if not compatibility.is_dir():
        return [f"missing compatibility directory: {compatibility_relative}"]

    failures: list[str] = []
    canonical_files = {
        path.relative_to(canonical)
        for path in canonical.rglob("*")
        if path.is_file()
    }
    compatibility_files = {
        path.relative_to(compatibility)
        for path in compatibility.rglob("*")
        if path.is_file()
    }
    for relative in sorted(canonical_files - compatibility_files):
        failures.append(f"missing compatibility copy: {compatibility_relative}/{relative}")
    for relative in sorted(compatibility_files - canonical_files):
        failures.append(f"stale compatibility file: {compatibility_relative}/{relative}")
    for relative in sorted(canonical_files & compatibility_files):
        canonical_file = canonical / relative
        compatibility_file = compatibility / relative
        if digest(canonical_file) != digest(compatibility_file):
            failures.append(
                f"content differs: {canonical_relative}/{relative} != "
                f"{compatibility_relative}/{relative}"
            )
    return failures


def main() -> int:
    log("info", "legacy-sync:check", "Comparing Sherpa canonical runtimes with compatibility copies")
    failures: list[str] = []
    for source, target in FILE_MAPPINGS:
        failures.extend(compare_file(source, target))
    for source, target in DIRECTORY_MAPPINGS:
        failures.extend(compare_directory(source, target))

    if failures:
        for failure in failures:
            log("error", "legacy-sync:check", failure)
        print(f"[legacy-sync:check:error] failures={len(failures)}", file=sys.stderr)
        return 1

    print("[legacy-sync:check:success] runtime sources are synchronized")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
