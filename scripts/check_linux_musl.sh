#!/usr/bin/env bash
# Linux musl cross-check (aligned with .github/workflows/rust.yml linux-musl-check).
# Uses stable toolchain so repo rust-toolchain.toml (esp) does not apply.
# If plain `cargo check` fails (linker), retries with cargo-zigbuild (single fallback).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

export RUSTUP_TOOLCHAIN=stable

rustup target add armv7-unknown-linux-musleabihf
rustup target add riscv64gc-unknown-linux-musl

check_target() {
  local t=$1
  if cargo check --target "$t"; then
    return 0
  fi
  echo "check_linux_musl: cargo check failed for $t; retrying with cargo-zigbuild" >&2
  if ! command -v cargo-zigbuild >/dev/null 2>&1; then
    cargo install cargo-zigbuild --locked
  fi
  cargo zigbuild --target "$t"
}

check_target armv7-unknown-linux-musleabihf
check_target riscv64gc-unknown-linux-musl

echo "OK: linux musl checks passed"
