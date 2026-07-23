#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
PLUGIN_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
INSTALL_ROOT="${SHERPA_INSTALL_ROOT:-$HOME/.local}"
COMPONENT="${1:-all}"
FAILURES=0
CALENDAR_ADAPTER_VERSION="0.1.2"
REMINDERS_ADAPTER_VERSION="1.5.1"
SHERPA_VERSION="0.1.0"
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
  local calendar_adapter=""
  local sherpa=""
  calendar_adapter="$(resolve_command sherpa-calendar-adapter)"
  sherpa="$(resolve_command sherpa)"
  echo "[doctor:calendar:start] Checking the Planner Calendar adapter" >&2
  if [ ! -x "$calendar_adapter" ]; then
    echo "[doctor:calendar:error] Calendar adapter is missing; run install-runtime.sh planner" >&2
    record_failure
    return
  fi
  if ! verify_version "$calendar_adapter" "$CALENDAR_ADAPTER_VERSION" "calendar"; then
    record_failure
    return
  fi
  if ! SHERPA_INSTALL_ROOT="$INSTALL_ROOT" "$sherpa" planner calendar doctor; then
    echo "[doctor:calendar:error] Calendar adapter reported an access problem" >&2
    record_failure
    return
  fi
  if ! "$sherpa" planner metadata spec >/dev/null; then
    echo "[doctor:calendar:error] Planner metadata schema check failed" >&2
    record_failure
    return
  fi
  echo "[doctor:calendar:success] Calendar capability is ready" >&2
}

doctor_reminders() {
  local reminders_adapter=""
  reminders_adapter="$(resolve_command sherpa-reminders-adapter)"
  echo "[doctor:reminders:start] Checking Reminders runtime and permission" >&2
  if [ ! -x "$reminders_adapter" ]; then
    echo "[doctor:reminders:error] Reminders adapter is missing; run install-runtime.sh planner" >&2
    record_failure
    return
  fi
  if ! verify_version "$reminders_adapter" "$REMINDERS_ADAPTER_VERSION" "reminders"; then
    record_failure
    return
  fi
  if [ "$reminders_adapter" = "$INSTALL_ROOT/bin/sherpa-reminders-adapter" ]; then
    local provenance_file="$INSTALL_ROOT/share/sherpa/reminders-adapter.provenance"
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
        echo "[doctor:reminders:error] Managed Reminders adapter is incomplete; run install-runtime.sh planner" >&2
        record_failure
        return
      fi
    done
    if [ ! -f "$provenance_file" ] \
      || ! grep -Fxq "version=$REMINDERS_ADAPTER_VERSION" "$provenance_file" \
      || ! grep -Fxq "commit=$REMCTL_COMMIT" "$provenance_file" \
      || ! grep -Fxq "source=$REMCTL_SOURCE" "$provenance_file"; then
      echo "[doctor:reminders:error] Managed Reminders adapter provenance differs; run install-runtime.sh planner" >&2
      record_failure
      return
    fi
    if ! cmp -s \
      "$INSTALL_ROOT/share/licenses/sherpa/remctl/LICENSE" \
      "$PLUGIN_ROOT/third_party/remctl/LICENSE"; then
      echo "[doctor:reminders:error] Managed Reminders adapter license differs; run install-runtime.sh planner" >&2
      record_failure
      return
    fi
  fi
  local doctor_json=""
  local doctor_exit=0
  local sherpa=""
  sherpa="$(resolve_command sherpa)"
  if doctor_json="$(
    SHERPA_INSTALL_ROOT="$INSTALL_ROOT" \
      "$sherpa" planner reminders doctor --for-agent --json 2>/dev/null
  )"; then
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

doctor_context() {
  local sherpa=""
  sherpa="$(resolve_command sherpa)"
  echo "[doctor:context:start] Checking the context runtime and optional sources" >&2
  if [ ! -x "$sherpa" ] || ! verify_version "$sherpa" "$SHERPA_VERSION" "context"; then
    echo "[doctor:context:error] sherpa is missing or incompatible; run install-runtime.sh context" >&2
    record_failure
    return
  fi
  if command -v fdesetup >/dev/null 2>&1 \
    && ! fdesetup status 2>/dev/null | grep -q 'FileVault is On'; then
    echo "[doctor:context:storage:warn] FileVault is not confirmed; collected context has weaker protection at rest" >&2
  fi
  if "$sherpa" context doctor kakao; then
    echo "[doctor:context:kakaotalk:warn] KakaoTalk source detected; authentication and database access are not verified" >&2
    local kakaocli=""
    kakaocli="$(resolve_command kakaocli)"
    if [ -x "$kakaocli" ] && "$kakaocli" help send >/dev/null 2>&1; then
      echo "[doctor:context:kakaotalk-reply:warn] Confirmation-gated replies are available; Accessibility and UI dispatch are not verified" >&2
    else
      echo "[doctor:context:kakaotalk-reply:warn] Installed kakaocli does not expose the required send command" >&2
    fi
  else
    echo "[doctor:context:kakaotalk:warn] KakaoTalk source is not configured" >&2
  fi
  if "$sherpa" context doctor imessage; then
    echo "[doctor:context:imessage:warn] iMessage source detected; Full Disk Access and database reads are not verified" >&2
  else
    echo "[doctor:context:imessage:warn] iMessage source is not configured" >&2
  fi
  echo "[doctor:context:success] Context capability is ready" >&2
}

doctor_planner() {
  local sherpa=""
  sherpa="$(resolve_command sherpa)"
  echo "[doctor:planner:start] Checking Planner and Apple adapters" >&2
  if [ ! -x "$sherpa" ] || ! verify_version "$sherpa" "$SHERPA_VERSION" "planner"; then
    echo "[doctor:planner:error] sherpa is missing or incompatible; run install-runtime.sh planner" >&2
    record_failure
    return
  fi
  doctor_calendar
  doctor_reminders
  echo "[doctor:planner:success] Planner checks completed" >&2
}

case "$COMPONENT" in
  -h|--help)
    echo "Usage: $0 <context|planner|all>" >&2
    exit 0
    ;;
  context) doctor_context ;;
  planner) doctor_planner ;;
  all)
    doctor_context
    doctor_planner
    ;;
  *)
    echo "Usage: $0 <context|planner|all>" >&2
    exit 2
    ;;
esac

if [ "$FAILURES" -gt 0 ]; then
  echo "[doctor:sherpa:error] failures=$FAILURES" >&2
  exit 1
fi

echo "[doctor:sherpa:success] component=$COMPONENT" >&2
