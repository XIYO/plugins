#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
PLUGIN_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
TEMP_ROOT="${TMPDIR:-/tmp}"
BUILD_DIR="$(mktemp -d "$TEMP_ROOT/apple-calendar-build.XXXXXX")"
INSTALL_ROOT="${APPLE_CALENDAR_INSTALL_ROOT:-$HOME/.local}"

cleanup() {
  case "$BUILD_DIR" in
    "$TEMP_ROOT"/apple-calendar-build.*) rm -rf -- "$BUILD_DIR" ;;
    *) echo "[install:cleanup:error] Refusing to remove unexpected build directory" >&2 ;;
  esac
}

trap cleanup EXIT

if [ "$(uname -s)" != "Darwin" ]; then
  echo "[install:platform:error] Apple Calendar runtime requires macOS" >&2
  exit 1
fi

for command_name in cargo swiftc codesign; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "[install:prerequisite:error] Missing required command: $command_name" >&2
    exit 1
  fi
done

echo "[install:calmeta:start] Installing the bundled Rust metadata runtime" >&2
CARGO_TARGET_DIR="$BUILD_DIR/cargo" cargo install \
  --path "$PLUGIN_ROOT" \
  --locked \
  --force \
  --root "$INSTALL_ROOT"
echo "[install:calmeta:success] Installed $INSTALL_ROOT/bin/calmeta" >&2

CALCTL_BUILD_BIN="$BUILD_DIR/calctl"
echo "[install:calctl:start] Building the EventKit adapter" >&2
swiftc -parse-as-library -O \
  -framework EventKit \
  -framework CoreGraphics \
  -Xlinker -sectcreate \
  -Xlinker __TEXT \
  -Xlinker __info_plist \
  -Xlinker "$PLUGIN_ROOT/runtime/Info.plist" \
  "$PLUGIN_ROOT/runtime/calctl.swift" \
  -o "$CALCTL_BUILD_BIN"

CALCTL_SIGN_IDENTITY="$(security find-identity -v -p codesigning 2>/dev/null | awk '/"Apple Development:/{print $2; exit}')"
if [ -n "$CALCTL_SIGN_IDENTITY" ]; then
  codesign --force --sign "$CALCTL_SIGN_IDENTITY" --identifier dev.xiyo.calctl "$CALCTL_BUILD_BIN"
else
  codesign --force --sign - --identifier dev.xiyo.calctl "$CALCTL_BUILD_BIN"
fi

mkdir -p "$INSTALL_ROOT/bin"
install -m 755 "$CALCTL_BUILD_BIN" "$INSTALL_ROOT/bin/calctl"
echo "[install:calctl:success] Installed $INSTALL_ROOT/bin/calctl" >&2
