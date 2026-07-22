#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
PLUGIN_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
REPOSITORY_ROOT="$(cd "$PLUGIN_ROOT/../.." && pwd -P)"
TEMP_ROOT="${TMPDIR:-/tmp}"
CHECK_TARGET_DIR="$(mktemp -d "$TEMP_ROOT/msgpipe-check.XXXXXX")"

cleanup() {
  case "$CHECK_TARGET_DIR" in
    "$TEMP_ROOT"/msgpipe-check.*) rm -rf -- "$CHECK_TARGET_DIR" ;;
    *) echo "[check:cleanup:error] Refusing to remove unexpected build directory" >&2 ;;
  esac
}

trap cleanup EXIT

cd "$PLUGIN_ROOT"

echo "[check:rust:start] Running formatter, linter, and tests" >&2
cargo fmt --all -- --check
CARGO_TARGET_DIR="$CHECK_TARGET_DIR" cargo clippy --all-targets --all-features -- -D warnings
CARGO_TARGET_DIR="$CHECK_TARGET_DIR" cargo test --all-targets --all-features
python3 "$REPOSITORY_ROOT/scripts/check-version-sync.py"
echo "[check:rust:success] Rust and version checks passed" >&2
