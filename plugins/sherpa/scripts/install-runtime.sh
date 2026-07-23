#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
PLUGIN_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
TEMP_ROOT="${TMPDIR:-/tmp}"
BUILD_ROOT="$(mktemp -d "$TEMP_ROOT/sherpa-install.XXXXXX")"
INSTALL_ROOT="${SHERPA_INSTALL_ROOT:-$HOME/.local}"
COMPONENT="${1:-all}"
REMCTL_TAG="v1.5.1"
REMCTL_COMMIT="eb75c451eab006218204bb78379917f3414fc6e3"
REMCTL_SOURCE="https://github.com/viticci/remctl.git"
CALMETA_VERSION="0.1.0"
CALCTL_VERSION="0.1.2"
MSGPIPE_VERSION="0.2.1"
SHERPA_VERSION="0.1.0"

cleanup() {
  case "$BUILD_ROOT" in
    "$TEMP_ROOT"/sherpa-install.*) rm -rf -- "$BUILD_ROOT" ;;
    *) echo "[install:sherpa:cleanup:error] Refusing to remove unexpected build directory" >&2 ;;
  esac
}

trap cleanup EXIT

usage() {
  echo "Usage: $0 <context|planner|all>" >&2
}

verify_version() {
  local binary="$1"
  local expected="$2"
  local label="$3"
  local output=""
  output="$("$binary" --version 2>/dev/null)"
  if ! printf '%s\n' "$output" | awk -v expected="$expected" '
    {
      for (field_index = 1; field_index <= NF; field_index += 1) {
        value = $field_index
        sub(/^v/, "", value)
        if (value == expected) found = 1
      }
    }
    END { exit(found ? 0 : 1) }
  '; then
    echo "[install:$label:verify:error] Expected version $expected" >&2
    return 1
  fi
}

require_command() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "[install:sherpa:prerequisite:error] Missing required command: $command_name" >&2
    return 1
  fi
}

install_calendar() {
  require_command cargo
  require_command swiftc
  require_command codesign

  echo "[install:calendar:start] Installing calmeta and calctl" >&2
  CARGO_TARGET_DIR="$BUILD_ROOT/calendar-cargo" cargo install \
    --path "$PLUGIN_ROOT/crates/calmeta" \
    --locked \
    --force \
    --root "$INSTALL_ROOT"

  local calctl_build_bin="$BUILD_ROOT/calctl"
  swiftc -parse-as-library -O \
    -framework EventKit \
    -framework CoreGraphics \
    -Xlinker -sectcreate \
    -Xlinker __TEXT \
    -Xlinker __info_plist \
    -Xlinker "$PLUGIN_ROOT/runtime/calctl/Info.plist" \
    "$PLUGIN_ROOT/runtime/calctl/calctl.swift" \
    -o "$calctl_build_bin"

  local sign_identity=""
  sign_identity="$(security find-identity -v -p codesigning 2>/dev/null | awk '/"Apple Development:/{print $2; exit}')"
  if [ -n "$sign_identity" ]; then
    codesign --force --sign "$sign_identity" --identifier dev.xiyo.calctl "$calctl_build_bin"
  else
    codesign --force --sign - --identifier dev.xiyo.calctl "$calctl_build_bin"
  fi

  mkdir -p "$INSTALL_ROOT/bin"
  install -m 755 "$calctl_build_bin" "$INSTALL_ROOT/bin/calctl"
  verify_version "$INSTALL_ROOT/bin/calmeta" "$CALMETA_VERSION" "calendar"
  verify_version "$INSTALL_ROOT/bin/calctl" "$CALCTL_VERSION" "calendar"
  echo "[install:calendar:success] Installed calmeta and calctl under $INSTALL_ROOT/bin" >&2
}

install_sherpa() {
  require_command cargo

  echo "[install:sherpa:start] Installing the unified Sherpa interface" >&2
  CARGO_TARGET_DIR="$BUILD_ROOT/sherpa-cargo" cargo install \
    --path "$PLUGIN_ROOT/crates/sherpa" \
    --locked \
    --force \
    --root "$INSTALL_ROOT"
  verify_version "$INSTALL_ROOT/bin/sherpa" "$SHERPA_VERSION" "sherpa"
  echo "[install:sherpa:success] Installed the unified interface under $INSTALL_ROOT/bin" >&2
}

install_reminders() {
  require_command git
  require_command python3
  require_command swiftc
  require_command clang

  local installed_remctl="$INSTALL_ROOT/bin/remctl"
  local provenance_file="$INSTALL_ROOT/share/sherpa/remctl.provenance"
  local required_file=""
  local verified_install=true
  for required_file in \
    "$installed_remctl" \
    "$INSTALL_ROOT/bin/remctl-bridge" \
    "$INSTALL_ROOT/bin/remctl-permissions" \
    "$INSTALL_ROOT/bin/remctl-private" \
    "$INSTALL_ROOT/bin/remctl_runtime.py" \
    "$INSTALL_ROOT/bin/remctl_images.py" \
    "$INSTALL_ROOT/bin/remctl_serialization.py" \
    "$INSTALL_ROOT/bin/remctl_smart_lists.py" \
    "$INSTALL_ROOT/share/licenses/sherpa/remctl/LICENSE"; do
    if [ ! -f "$required_file" ]; then
      verified_install=false
    fi
  done
  if [ ! -f "$provenance_file" ] \
    || ! grep -Fxq "version=${REMCTL_TAG#v}" "$provenance_file" \
    || ! grep -Fxq "commit=$REMCTL_COMMIT" "$provenance_file" \
    || ! grep -Fxq "source=$REMCTL_SOURCE" "$provenance_file"; then
    verified_install=false
  fi
  if [ "$verified_install" = true ] \
    && ! cmp -s \
      "$INSTALL_ROOT/share/licenses/sherpa/remctl/LICENSE" \
      "$PLUGIN_ROOT/third_party/remctl/LICENSE"; then
    verified_install=false
  fi
  if [ "$verified_install" = true ] \
    && verify_version "$installed_remctl" "${REMCTL_TAG#v}" "reminders" >/dev/null 2>&1; then
    echo "[install:reminders:success] Verified RemCTL ${REMCTL_TAG#v} installation is already present" >&2
    return 0
  fi

  local source_dir="$BUILD_ROOT/remctl"
  echo "[install:reminders:start] Fetching verified RemCTL $REMCTL_TAG" >&2
  git clone --quiet --depth 1 --branch "$REMCTL_TAG" "$REMCTL_SOURCE" "$source_dir"

  local actual_commit=""
  actual_commit="$(git -C "$source_dir" rev-parse HEAD)"
  if [ "$actual_commit" != "$REMCTL_COMMIT" ]; then
    echo "[install:reminders:verify:error] RemCTL commit mismatch" >&2
    echo "[install:reminders:verify:error] expected=$REMCTL_COMMIT actual=$actual_commit" >&2
    return 1
  fi
  if ! cmp -s "$source_dir/LICENSE" "$PLUGIN_ROOT/third_party/remctl/LICENSE"; then
    echo "[install:reminders:verify:error] RemCTL license differs from the reviewed copy" >&2
    return 1
  fi

  local stage_root="$BUILD_ROOT/remctl-stage"
  local stage_bin="$stage_root/bin"
  local stage_config="$BUILD_ROOT/remctl-config"
  local upstream_log="$BUILD_ROOT/remctl-install.log"
  if ! PATH="$stage_bin:$PATH" PREFIX="$stage_root" REMCTL_CONFIG_DIR="$stage_config" \
    bash "$source_dir/install.sh" --shell-completions none >"$upstream_log" 2>&1; then
    echo "[install:reminders:upstream:error] RemCTL staging failed; temporary upstream log follows" >&2
    sed -n '1,200p' "$upstream_log" >&2
    return 1
  fi

  for required_file in \
    remctl remctl-bridge remctl-permissions remctl-private \
    remctl_runtime.py remctl_images.py remctl_serialization.py remctl_smart_lists.py; do
    if [ ! -f "$stage_bin/$required_file" ]; then
      echo "[install:reminders:verify:error] Missing staged RemCTL component: $required_file" >&2
      return 1
    fi
  done

  mkdir -p "$INSTALL_ROOT/bin"
  install -m 755 "$stage_bin/remctl" "$INSTALL_ROOT/bin/remctl"
  install -m 755 "$stage_bin/remctl-bridge" "$INSTALL_ROOT/bin/remctl-bridge"
  install -m 755 "$stage_bin/remctl-permissions" "$INSTALL_ROOT/bin/remctl-permissions"
  install -m 755 "$stage_bin/remctl-private" "$INSTALL_ROOT/bin/remctl-private"
  install -m 644 "$stage_bin/remctl_runtime.py" "$INSTALL_ROOT/bin/remctl_runtime.py"
  install -m 644 "$stage_bin/remctl_images.py" "$INSTALL_ROOT/bin/remctl_images.py"
  install -m 644 "$stage_bin/remctl_serialization.py" "$INSTALL_ROOT/bin/remctl_serialization.py"
  install -m 644 "$stage_bin/remctl_smart_lists.py" "$INSTALL_ROOT/bin/remctl_smart_lists.py"
  if [ -f "$stage_bin/remctl-permissions-icon.png" ]; then
    install -m 644 "$stage_bin/remctl-permissions-icon.png" "$INSTALL_ROOT/bin/remctl-permissions-icon.png"
  fi

  local provenance_source="$BUILD_ROOT/remctl.provenance"
  printf 'version=%s\ncommit=%s\nsource=%s\n' \
    "${REMCTL_TAG#v}" "$REMCTL_COMMIT" "$REMCTL_SOURCE" > "$provenance_source"
  install -d -m 700 "$INSTALL_ROOT/share/sherpa"
  install -m 600 "$provenance_source" "$provenance_file"
  install -d -m 755 "$INSTALL_ROOT/share/licenses/sherpa/remctl"
  install -m 644 "$source_dir/LICENSE" "$INSTALL_ROOT/share/licenses/sherpa/remctl/LICENSE"

  verify_version "$installed_remctl" "${REMCTL_TAG#v}" "reminders"
  echo "[install:reminders:success] Installed verified RemCTL components without upstream aliases" >&2
  echo "[install:reminders:notice] No rctl/reminders aliases or shell configuration were installed" >&2
}

install_context_engine() {
  require_command cargo

  echo "[install:context:start] Installing the context engine" >&2
  CARGO_TARGET_DIR="$BUILD_ROOT/context-cargo" cargo install \
    --path "$PLUGIN_ROOT/crates/msgpipe" \
    --locked \
    --force \
    --root "$INSTALL_ROOT"
  verify_version "$INSTALL_ROOT/bin/msgpipe" "$MSGPIPE_VERSION" "context"
  echo "[install:context:success] Installed the internal context engine" >&2

  if ! command -v kakaocli >/dev/null 2>&1; then
    echo "[install:context:source:warn] kakaocli is optional and was not installed" >&2
  fi
  if ! command -v imsg >/dev/null 2>&1; then
    echo "[install:context:source:warn] imsg is optional and was not installed" >&2
  fi
}

install_planner() {
  install_calendar
  install_reminders
}

if [ "$COMPONENT" = "--help" ] || [ "$COMPONENT" = "-h" ]; then
  usage
  exit 0
fi

if [ "$(uname -s)" != "Darwin" ]; then
  echo "[install:sherpa:platform:error] Sherpa runtimes require macOS" >&2
  exit 1
fi

case "$COMPONENT" in
  context)
    install_sherpa
    install_context_engine
    ;;
  planner)
    install_sherpa
    install_planner
    ;;
  all)
    install_sherpa
    install_context_engine
    install_planner
    ;;
  # Compatibility aliases for installations created before the domain migration.
  calendar)
    install_sherpa
    install_calendar
    ;;
  reminders)
    install_sherpa
    install_reminders
    ;;
  messages)
    install_sherpa
    install_context_engine
    ;;
  *)
    usage
    exit 2
    ;;
esac

echo "[install:sherpa:success] component=$COMPONENT" >&2
