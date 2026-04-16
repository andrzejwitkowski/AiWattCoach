#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
graphify_python_default="$HOME/.local/pipx/venvs/graphifyy/bin/python"
graphify_python_override_file="$repo_root/graphify-out/.graphify_python"

graphify_python="${GRAPHIFY_PYTHON:-}"

if [ -z "$graphify_python" ] && [ -f "$graphify_python_override_file" ]; then
  override_python="$(tr -d '\r' < "$graphify_python_override_file")"
  if [ -n "$override_python" ] && [ -x "$override_python" ]; then
    graphify_python="$override_python"
  fi
fi

if [ -z "$graphify_python" ] && [ -x "$graphify_python_default" ]; then
  graphify_python="$graphify_python_default"
fi

if [ -z "$graphify_python" ]; then
  echo "graphify python not found. Install graphifyy with 'pipx install graphifyy' or set GRAPHIFY_PYTHON." >&2
  exit 1
fi

cd "$repo_root"
"$graphify_python" -c "from graphify.watch import _rebuild_code; from pathlib import Path; _rebuild_code(Path('.'))"
