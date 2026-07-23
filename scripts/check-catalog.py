#!/usr/bin/env python3

from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parent.parent
PLUGIN_ROOT = ROOT / "plugins"
CODEX_MARKETPLACE = ROOT / ".agents" / "plugins" / "marketplace.json"
CLAUDE_MARKETPLACE = ROOT / ".claude-plugin" / "marketplace.json"
README_PATHS = (ROOT / "README.md", ROOT / "README.ko.md")
CATALOG_POLICY = ROOT / "catalog-policy.json"
SHERPA_REQUIRED_SKILLS = {
    "sherpa",
    "context",
    "planner",
}
VALID_LOG_LEVELS = {"debug": 10, "info": 20, "warn": 30, "error": 40}
LOG_LEVEL = VALID_LOG_LEVELS.get(os.getenv("LOG_LEVEL", "warn").lower(), 30)


def log(level: str, scope: str, message: str) -> None:
    if VALID_LOG_LEVELS[level] >= LOG_LEVEL:
        print(f"[{scope}:{level}] {message}", file=sys.stderr)


def fail(message: str) -> None:
    log("error", "catalog:validate", message)
    raise ValueError(message)


def read_json(path: Path) -> dict:
    log("debug", "catalog:read", f"Reading {path.relative_to(ROOT)}")
    with path.open(encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        fail(f"Expected an object in {path.relative_to(ROOT)}")
    return value


def marketplace_names(document: dict, path: Path) -> list[str]:
    plugins = document.get("plugins")
    if not isinstance(plugins, list) or not plugins:
        fail(f"No plugins found in {path.relative_to(ROOT)}")

    names: list[str] = []
    for index, entry in enumerate(plugins):
        if not isinstance(entry, dict) or not isinstance(entry.get("name"), str):
            fail(f"Invalid plugin entry {index} in {path.relative_to(ROOT)}")
        names.append(entry["name"])

    if len(names) != len(set(names)):
        fail(f"Duplicate plugin names in {path.relative_to(ROOT)}")
    return names


def validate_plugin(name: str, codex_entry: dict, claude_entry: dict) -> None:
    expected_path = PLUGIN_ROOT / name
    if not expected_path.is_dir():
        fail(f"Marketplace plugin directory is missing: plugins/{name}")

    codex_source = codex_entry.get("source", {})
    if codex_source.get("source") != "local":
        fail(f"Codex source for {name} must be local")
    if codex_source.get("path") != f"./plugins/{name}":
        fail(f"Codex source path for {name} is not canonical")
    if claude_entry.get("source") != f"./plugins/{name}":
        fail(f"Claude source path for {name} is not canonical")

    for host in ("codex", "claude"):
        manifest_path = expected_path / f".{host}-plugin" / "plugin.json"
        manifest = read_json(manifest_path)
        if manifest.get("name") != name:
            fail(f"Manifest name mismatch in {manifest_path.relative_to(ROOT)}")
        if not isinstance(manifest.get("version"), str):
            fail(f"Manifest version is missing in {manifest_path.relative_to(ROOT)}")

    skill_path = expected_path / "skills" / name / "SKILL.md"
    if not skill_path.is_file():
        fail(f"Skill entry point is missing: {skill_path.relative_to(ROOT)}")

    for bundled_skill_path in sorted((expected_path / "skills").glob("*/SKILL.md")):
        validate_skill(bundled_skill_path)

    install_id = f"{name}@xiyo"
    for readme_path in README_PATHS:
        if install_id not in readme_path.read_text(encoding="utf-8"):
            fail(f"{install_id} is missing from {readme_path.name}")

    if name == "sherpa":
        skill_names = {
            path.parent.name for path in (expected_path / "skills").glob("*/SKILL.md")
        }
        if skill_names != SHERPA_REQUIRED_SKILLS:
            fail(
                "Sherpa bundled skill set differs: "
                f"expected={sorted(SHERPA_REQUIRED_SKILLS)}, actual={sorted(skill_names)}"
            )
        required_paths = (
            expected_path / "LICENSE",
            expected_path / "runtime-versions.json",
            expected_path / "scripts" / "install-runtime.sh",
            expected_path / "scripts" / "doctor.sh",
            expected_path / "crates" / "sherpa" / "Cargo.toml",
            expected_path / "crates" / "context-engine" / "Cargo.toml",
            expected_path / "crates" / "planner-metadata" / "Cargo.toml",
            expected_path
            / "runtime"
            / "calendar-adapter"
            / "calendar-adapter.swift",
            expected_path / "third_party" / "remctl" / "LICENSE",
        )
        for required_path in required_paths:
            if not required_path.is_file():
                fail(f"Sherpa component is missing: {required_path.relative_to(ROOT)}")


def validate_skill(path: Path) -> None:
    text = path.read_text(encoding="utf-8")
    frontmatter = text.split("---", 2)
    if len(frontmatter) < 3:
        fail(f"Skill frontmatter is missing: {path.relative_to(ROOT)}")

    expected_name = path.parent.name
    name_match = re.search(r"(?m)^name:\s*([^\n]+?)\s*$", frontmatter[1])
    description_match = re.search(
        r"(?m)^description:\s*(.+?)\s*$", frontmatter[1]
    )
    if name_match is None or name_match.group(1) != expected_name:
        fail(f"Skill name mismatch in {path.relative_to(ROOT)}")
    if description_match is None:
        fail(f"Skill description is missing in {path.relative_to(ROOT)}")
    description = description_match.group(1)
    if len(description) > 1_024:
        fail(f"Skill description exceeds 1024 characters: {path.relative_to(ROOT)}")
    if "<" in description or ">" in description:
        fail(f"Skill description contains angle brackets: {path.relative_to(ROOT)}")

    agent_metadata = path.parent / "agents" / "openai.yaml"
    if path.parts[-4:-3] == ("sherpa",) and not agent_metadata.is_file():
        fail(f"Sherpa skill agent metadata is missing: {agent_metadata.relative_to(ROOT)}")


def validate_relative_links(path: Path) -> None:
    text = path.read_text(encoding="utf-8")
    for raw_target in re.findall(r"(?<!!)\[[^\]]+\]\(([^)]+)\)", text):
        target = raw_target.strip().split(maxsplit=1)[0].strip("<>")
        parsed = urlsplit(target)
        if parsed.scheme or target.startswith("#"):
            continue
        relative_target = unquote(parsed.path)
        resolved = (path.parent / relative_target).resolve()
        if not resolved.exists():
            fail(
                f"Broken relative link in {path.relative_to(ROOT)}: {relative_target}"
            )


def main() -> int:
    log("info", "catalog:validate", "Starting marketplace and README checks")
    codex = read_json(CODEX_MARKETPLACE)
    claude = read_json(CLAUDE_MARKETPLACE)

    codex_names = marketplace_names(codex, CODEX_MARKETPLACE)
    claude_names = marketplace_names(claude, CLAUDE_MARKETPLACE)
    if codex_names != claude_names:
        fail("Codex and Claude marketplace order or membership differs")

    policy = read_json(CATALOG_POLICY)
    primary = policy.get("primary")
    if not isinstance(primary, str):
        fail("catalog-policy.json must define primary")
    if codex_names[0] != primary:
        fail(f"Primary plugin must be first: expected={primary}, actual={codex_names[0]}")
    if codex_names != [primary]:
        fail(f"Only the primary plugin may be published: marketplace={codex_names}")

    directory_names = sorted(
        path.name for path in PLUGIN_ROOT.iterdir() if path.is_dir()
    )
    if sorted(codex_names) != directory_names:
        fail(
            "Plugin directories and marketplace membership differ: "
            f"directories={directory_names}, marketplace={sorted(codex_names)}"
        )

    codex_entries = {entry["name"]: entry for entry in codex["plugins"]}
    claude_entries = {entry["name"]: entry for entry in claude["plugins"]}
    for name in codex_names:
        validate_plugin(name, codex_entries[name], claude_entries[name])

    markdown_paths = list(README_PATHS) + sorted(PLUGIN_ROOT.glob("**/*.md"))
    for markdown_path in markdown_paths:
        validate_relative_links(markdown_path)

    print(
        f"[catalog:validate:success] plugins={len(codex_names)} "
        f"readmes={len(markdown_paths)}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, json.JSONDecodeError) as error:
        log("error", "catalog:validate", "Validation failed")
        print(error, file=sys.stderr)
        raise SystemExit(1) from error
