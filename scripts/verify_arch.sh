#!/usr/bin/env sh

set -eu

TOOLCHAIN="nightly-2026-01-22"

if ! rustup toolchain list | grep -q "^${TOOLCHAIN}"; then
  echo "Missing Rust toolchain: ${TOOLCHAIN}" >&2
  echo "Install it with:" >&2
  echo "  rustup toolchain install ${TOOLCHAIN}" >&2
  exit 1
fi

for component in rust-src rustc-dev llvm-tools; do
  if ! rustup component list --toolchain "${TOOLCHAIN}" | grep -q "^${component}.*(installed)"; then
    echo "Missing Rust component '${component}' for ${TOOLCHAIN}" >&2
    echo "Install required components with:" >&2
    echo "  rustup component add --toolchain ${TOOLCHAIN} rust-src rustc-dev llvm-tools-preview" >&2
    exit 1
  fi
done

if ! command -v cargo-pup >/dev/null 2>&1; then
  echo "Missing cargo_pup CLI" >&2
  echo "Install it with:" >&2
  echo "  cargo +${TOOLCHAIN} install cargo_pup --version 0.1.7 --locked" >&2
  exit 1
fi

cargo-pup check
