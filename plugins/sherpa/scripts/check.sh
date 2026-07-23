#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
PLUGIN_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
REPOSITORY_ROOT="$(cd "$PLUGIN_ROOT/../.." && pwd -P)"
TEMP_ROOT="${TMPDIR:-/tmp}"
CHECK_TARGET_DIR="$(mktemp -d "$TEMP_ROOT/sherpa-check.XXXXXX")"

cleanup() {
  case "$CHECK_TARGET_DIR" in
    "$TEMP_ROOT"/sherpa-check.*) rm -rf -- "$CHECK_TARGET_DIR" ;;
    *) echo "[check:sherpa:cleanup:error] Refusing to remove unexpected build directory" >&2 ;;
  esac
}

trap cleanup EXIT

cd "$PLUGIN_ROOT"

echo "[check:sherpa:rust:start] Running workspace formatter, linter, and tests" >&2
cargo fmt --all -- --check
CARGO_TARGET_DIR="$CHECK_TARGET_DIR" cargo clippy --workspace --all-targets --all-features -- -D warnings
CARGO_TARGET_DIR="$CHECK_TARGET_DIR" cargo test --workspace --all-targets --all-features
echo "[check:sherpa:rust:success] Rust workspace passed" >&2

echo "[check:sherpa:swift:start] Type-checking the EventKit adapter" >&2
swiftc -parse-as-library -typecheck \
  -framework EventKit \
  -framework CoreGraphics \
  runtime/calctl/calctl.swift
echo "[check:sherpa:swift:success] EventKit adapter passed" >&2

bash -n scripts/install-runtime.sh scripts/doctor.sh scripts/check.sh
bash scripts/install-runtime.sh --help >/dev/null
bash scripts/doctor.sh --help >/dev/null
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests -p 'test_*.py'
python3 scripts/kakao-reply.py --help >/dev/null

echo "[check:sherpa:install:start] Smoke-testing all managed runtimes in an isolated root" >&2
SMOKE_INSTALL_ROOT="$CHECK_TARGET_DIR/install-root"
SHERPA_INSTALL_ROOT="$SMOKE_INSTALL_ROOT" bash scripts/install-runtime.sh all
test "$("$SMOKE_INSTALL_ROOT/bin/calctl" --version)" = "0.1.2"
test "$("$SMOKE_INSTALL_ROOT/bin/calmeta" --version)" = "calmeta 0.1.0"
test "$("$SMOKE_INSTALL_ROOT/bin/msgpipe" --version)" = "msgpipe 0.2.1"
test "$("$SMOKE_INSTALL_ROOT/bin/remctl" --version)" = "1.5.1"
test -f "$SMOKE_INSTALL_ROOT/share/sherpa/remctl.provenance"
cmp -s \
  "$SMOKE_INSTALL_ROOT/share/licenses/sherpa/remctl/LICENSE" \
  third_party/remctl/LICENSE
test ! -e "$SMOKE_INSTALL_ROOT/bin/rctl"
test ! -e "$SMOKE_INSTALL_ROOT/bin/reminders"
printf '%s\n' 'tampered test copy' >"$SMOKE_INSTALL_ROOT/share/licenses/sherpa/remctl/LICENSE"
SHERPA_INSTALL_ROOT="$SMOKE_INSTALL_ROOT" bash scripts/install-runtime.sh reminders
cmp -s \
  "$SMOKE_INSTALL_ROOT/share/licenses/sherpa/remctl/LICENSE" \
  third_party/remctl/LICENSE
echo "[check:sherpa:install:success] Isolated runtime installation passed" >&2

python3 "$REPOSITORY_ROOT/scripts/check-legacy-sync.py"
python3 "$REPOSITORY_ROOT/scripts/check-version-sync.py"
echo "[check:sherpa:success] Sherpa checks passed" >&2
