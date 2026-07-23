#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
PLUGIN_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
INSTALL_ROOT="${SHERPA_INSTALL_ROOT:-$HOME/.local}"
COMPONENT="${1:-all}"
FAILURES=0
CALMETA_VERSION="0.1.0"
CALCTL_VERSION="0.1.2"
MSGPIPE_VERSION="0.2.1"
REMCTL_VERSION="1.5.1"
REMCTL_COMMIT="eb75c451eab006218204bb78379917f3414fc6e3"
REMCTL_SOURCE="https://github.com/viticci/remctl.git"

resolve_command() {
  local command_name="$1"
  local managed="$INSTALL_ROOT/bin/$command_name"
  local discovered=""
  if [ -x "$managed" ]; then
    printf '%s' "$managed"
    return
  fi
  discovered="$(command -v "$command_name" || true)"
  if [ -n "$discovered" ]; then
    printf '%s' "$discovered"
  else
    printf '%s/bin/%s' "$INSTALL_ROOT" "$command_name"
  fi
}

verify_version() {
  local binary="$1"
  local expected="$2"
  local label="$3"
  local output=""
  output="$("$binary" --version 2>/dev/null)"
  if printf '%s\n' "$output" | awk -v expected="$expected" '
    {
      for (field_index = 1; field_index <= NF; field_index += 1) {
        value = $field_index
        sub(/^v/, "", value)
        if (value == expected) found = 1
      }
    }
    END { exit(found ? 0 : 1) }
  '; then
    return 0
  fi
  echo "[doctor:$label:error] Runtime version mismatch; expected=$expected" >&2
  return 1
}

record_failure() {
  FAILURES=$((FAILURES + 1))
}

doctor_calendar() {
  local calctl=""
  local calmeta=""
  calctl="$(resolve_command calctl)"
  calmeta="$(resolve_command calmeta)"
  echo "[doctor:calendar:start] Checking Calendar runtimes and permission" >&2
  if [ ! -x "$calctl" ] || [ ! -x "$calmeta" ]; then
    echo "[doctor:calendar:error] calctl or calmeta is missing; run install-runtime.sh calendar" >&2
    record_failure
    return
  fi
  if ! verify_version "$calctl" "$CALCTL_VERSION" "calendar" \
    || ! verify_version "$calmeta" "$CALMETA_VERSION" "calendar"; then
    record_failure
    return
  fi
  if ! "$calctl" doctor; then
    echo "[doctor:calendar:error] calctl reported an access problem" >&2
    record_failure
    return
  fi
  if ! "$calmeta" spec >/dev/null; then
    echo "[doctor:calendar:error] calmeta schema check failed" >&2
    record_failure
    return
  fi
  echo "[doctor:calendar:success] Calendar capability is ready" >&2
}

doctor_reminders() {
  local remctl=""
  remctl="$(resolve_command remctl)"
  echo "[doctor:reminders:start] Checking Reminders runtime and permission" >&2
  if [ ! -x "$remctl" ]; then
    echo "[doctor:reminders:error] remctl is missing; run install-runtime.sh reminders" >&2
    record_failure
    return
  fi
  if ! verify_version "$remctl" "$REMCTL_VERSION" "reminders"; then
    record_failure
    return
  fi
  if [ "$remctl" = "$INSTALL_ROOT/bin/remctl" ]; then
    local provenance_file="$INSTALL_ROOT/share/sherpa/remctl.provenance"
    local required_file=""
    for required_file in \
      "$INSTALL_ROOT/bin/remctl-bridge" \
      "$INSTALL_ROOT/bin/remctl-permissions" \
      "$INSTALL_ROOT/bin/remctl-private" \
      "$INSTALL_ROOT/bin/remctl_runtime.py" \
      "$INSTALL_ROOT/bin/remctl_images.py" \
      "$INSTALL_ROOT/bin/remctl_serialization.py" \
      "$INSTALL_ROOT/bin/remctl_smart_lists.py" \
      "$INSTALL_ROOT/share/licenses/sherpa/remctl/LICENSE"; do
      if [ ! -f "$required_file" ]; then
        echo "[doctor:reminders:error] Managed RemCTL installation is incomplete; run install-runtime.sh reminders" >&2
        record_failure
        return
      fi
    done
    if [ ! -f "$provenance_file" ] \
      || ! grep -Fxq "version=$REMCTL_VERSION" "$provenance_file" \
      || ! grep -Fxq "commit=$REMCTL_COMMIT" "$provenance_file" \
      || ! grep -Fxq "source=$REMCTL_SOURCE" "$provenance_file"; then
      echo "[doctor:reminders:error] Managed RemCTL provenance is missing or differs; run install-runtime.sh reminders" >&2
      record_failure
      return
    fi
    if ! cmp -s \
      "$INSTALL_ROOT/share/licenses/sherpa/remctl/LICENSE" \
      "$PLUGIN_ROOT/third_party/remctl/LICENSE"; then
      echo "[doctor:reminders:error] Managed RemCTL license notice differs; run install-runtime.sh reminders" >&2
      record_failure
      return
    fi
  else
    echo "[doctor:reminders:warn] Using an external RemCTL binary; source provenance is not managed by Sherpa" >&2
  fi
  local doctor_json=""
  local doctor_exit=0
  if doctor_json="$("$remctl" doctor --for-agent --json 2>/dev/null)"; then
    doctor_exit=0
  else
    doctor_exit=$?
  fi
  local doctor_summary=""
  if ! doctor_summary="$(printf '%s' "$doctor_json" | python3 -c '
import json
import sys

report = json.load(sys.stdin)
print(f"{int(report.get('"'"'failures'"'"', 0))} {int(report.get('"'"'warnings'"'"', 0))}")
' 2>/dev/null)"; then
    echo "[doctor:reminders:error] RemCTL returned an unreadable diagnostic report" >&2
    record_failure
    return
  fi
  local doctor_failures=""
  local doctor_warnings=""
  read -r doctor_failures doctor_warnings <<< "$doctor_summary"
  if [ "$doctor_exit" -ne 0 ] || [ "$doctor_failures" -gt 0 ]; then
    echo "[doctor:reminders:error] RemCTL setup checks failed; failures=$doctor_failures warnings=$doctor_warnings" >&2
    record_failure
    return
  fi
  if [ "$doctor_warnings" -gt 0 ]; then
    echo "[doctor:reminders:warn] RemCTL reported optional setup warnings; count=$doctor_warnings" >&2
  fi
  echo "[doctor:reminders:success] Reminders capability is ready" >&2
}

doctor_messages() {
  local msgpipe=""
  msgpipe="$(resolve_command msgpipe)"
  echo "[doctor:messages:start] Checking message runtime and optional readers" >&2
  if [ ! -x "$msgpipe" ]; then
    echo "[doctor:messages:error] msgpipe is missing; run install-runtime.sh messages" >&2
    record_failure
    return
  fi
  if ! verify_version "$msgpipe" "$MSGPIPE_VERSION" "messages"; then
    record_failure
    return
  fi
  if command -v fdesetup >/dev/null 2>&1 \
    && ! fdesetup status 2>/dev/null | grep -q 'FileVault is On'; then
    echo "[doctor:messages:storage:warn] FileVault is not confirmed; local message content has weaker protection at rest" >&2
  fi
  if "$msgpipe" doctor kakao; then
    echo "[doctor:messages:kakao:warn] KakaoTalk reader executable detected; authentication and database access are not verified" >&2
  else
    echo "[doctor:messages:kakao:warn] KakaoTalk reader is not configured" >&2
  fi
  if "$msgpipe" doctor imessage; then
    echo "[doctor:messages:imessage:warn] iMessage reader executable detected; Full Disk Access and database reads are not verified" >&2
  else
    echo "[doctor:messages:imessage:warn] iMessage reader is not configured" >&2
  fi
  echo "[doctor:messages:success] msgpipe runtime is ready" >&2
}

case "$COMPONENT" in
  -h|--help)
    echo "Usage: $0 <calendar|reminders|messages|all>" >&2
    exit 0
    ;;
  calendar) doctor_calendar ;;
  reminders) doctor_reminders ;;
  messages) doctor_messages ;;
  all)
    doctor_calendar
    doctor_reminders
    doctor_messages
    ;;
  *)
    echo "Usage: $0 <calendar|reminders|messages|all>" >&2
    exit 2
    ;;
esac

if [ "$FAILURES" -gt 0 ]; then
  echo "[doctor:sherpa:error] failures=$FAILURES" >&2
  exit 1
fi

echo "[doctor:sherpa:success] component=$COMPONENT" >&2
