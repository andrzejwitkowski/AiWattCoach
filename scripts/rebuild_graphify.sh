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
GRAPHIFY_PROJECT_NAME="${GRAPHIFY_PROJECT_NAME:-AiWattCoach}" \
"$graphify_python" -c "import os; from graphify.watch import _rebuild_code; from pathlib import Path; os.environ.setdefault('GRAPHIFY_PROJECT_NAME', os.environ.get('GRAPHIFY_PROJECT_NAME', 'AiWattCoach')); _rebuild_code(Path.cwd())"

report_path="$repo_root/graphify-out/GRAPH_REPORT.md"
if [ -f "$report_path" ]; then
  "$graphify_python" - "$report_path" "${repo_root##*/}" <<'PY'
from pathlib import Path
import re
import sys

report_path = Path(sys.argv[1])
repo_name = sys.argv[2]
text = report_path.read_text(encoding="utf-8")
lines = text.splitlines()

if lines and lines[0].startswith("# Graph Report - "):
    lines[0] = re.sub(r"^# Graph Report - .*?  \(", f"# Graph Report - {repo_name}  (", lines[0], count=1)
    suffix = "\n" if text.endswith("\n") else ""
    report_path.write_text("\n".join(lines) + suffix, encoding="utf-8")
PY
fi
