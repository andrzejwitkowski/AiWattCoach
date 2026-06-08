# Git hooks from GUI clients often run with a minimal PATH.
if [ -z "${BUN_INSTALL:-}" ]; then
  BUN_INSTALL="$HOME/.bun"
fi

export PATH="$BUN_INSTALL/bin:$HOME/.cargo/bin:$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

if ! command -v bun >/dev/null 2>&1; then
  echo "error: bun not found in PATH" >&2
  echo "Install bun or add it to PATH before pushing." >&2
  exit 127
fi
