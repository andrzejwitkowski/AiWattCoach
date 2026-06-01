#!/usr/bin/env sh

set -eu

ERRORS=0
SRC="src"

check_no_import() {
  dir="$1"
  pattern="$2"
  label="$3"
  exclude="${4:-}"

  matches=$(grep -rn "$pattern" "$dir" --include='*.rs' 2>/dev/null || true)

  if [ -n "$exclude" ]; then
    # shellcheck disable=SC2001
    matches=$(echo "$matches" | sed "\#$exclude#d" || true)
  fi

  if [ -n "$matches" ]; then
    echo "FAIL: $label"
    echo "$matches" | while IFS= read -r line; do
      echo "  $line"
    done
    ERRORS=$((ERRORS + 1))
  fi
}

# 1. domain must not import adapters
check_no_import "$SRC/domain" '(crate::)?adapters::' "domain imports adapters"

# 2. domain must not import config
check_no_import "$SRC/domain" '(crate::)?config::' "domain imports config"

# 3. rest must not import mongo / intervals_icu / llm / google_oauth
# NOTE: health.rs probes mongo directly for readiness checks — known exception
check_no_import "$SRC/adapters/rest" 'adapters::mongo' "rest imports mongo" "health\.rs"
check_no_import "$SRC/adapters/rest" 'adapters::intervals_icu' "rest imports intervals_icu"
check_no_import "$SRC/adapters/rest" 'adapters::llm' "rest imports llm"
check_no_import "$SRC/adapters/rest" 'adapters::google_oauth' "rest imports google_oauth"

# 4. infra adapters must not import rest
check_no_import "$SRC/adapters/mongo" 'adapters::rest' "mongo imports rest"
check_no_import "$SRC/adapters/intervals_icu" 'adapters::rest' "intervals_icu imports rest"
check_no_import "$SRC/adapters/llm" 'adapters::rest' "llm imports rest"
check_no_import "$SRC/adapters/google_oauth" 'adapters::rest' "google_oauth imports rest"

if [ "$ERRORS" -gt 0 ]; then
  echo "---"
  echo "FAILED: $ERRORS architecture violation(s) found."
  exit 1
fi

echo "PASS: architecture boundaries are clean."
