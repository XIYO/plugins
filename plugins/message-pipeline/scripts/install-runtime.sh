#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
PLUGIN_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
TEMP_ROOT="${TMPDIR:-/tmp}"
BUILD_DIR="$(mktemp -d "$TEMP_ROOT/msgpipe-build.XXXXXX")"

cleanup() {
  case "$BUILD_DIR" in
    "$TEMP_ROOT"/msgpipe-build.*) rm -rf -- "$BUILD_DIR" ;;
    *) echo "[install:msgpipe:error] Refusing to remove unexpected build directory" >&2 ;;
  esac
}

trap cleanup EXIT

if ! command -v cargo >/dev/null 2>&1; then
  echo "[install:msgpipe:error] Rust cargo is required" >&2
  exit 1
fi

echo "[install:msgpipe:start] Installing the bundled msgpipe runtime" >&2
CARGO_TARGET_DIR="$BUILD_DIR" cargo install --path "$PLUGIN_ROOT" --locked --force
echo "[install:msgpipe:success] Installed msgpipe" >&2

if ! command -v kakaocli >/dev/null 2>&1; then
  echo "[install:reader:warn] kakaocli is not installed; use the bundled x-kakaotalk skill" >&2
fi

if ! command -v imsg >/dev/null 2>&1; then
  echo "[install:reader:warn] imsg is not installed; use the bundled x-imessage skill" >&2
fi
