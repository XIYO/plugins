#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPOSITORY_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"

echo "[check:repository:start] Validating marketplace catalog and documentation" >&2
python3 "$SCRIPT_DIR/check-catalog.py"
python3 "$SCRIPT_DIR/check-version-sync.py"
echo "[check:repository:success] Repository checks passed" >&2

while IFS= read -r check_script; do
  plugin_name="$(basename "$(dirname "$(dirname "$check_script")")")"
  echo "[check:plugin:start] plugin=$plugin_name" >&2
  bash "$check_script"
  echo "[check:plugin:success] plugin=$plugin_name" >&2
done < <(find "$REPOSITORY_ROOT/plugins" -mindepth 3 -maxdepth 3 -path '*/scripts/check.sh' -type f | sort)
