#!/usr/bin/env bash
# Build beetle-${VERSION}-linux-{armv7,riscv64}-musl.tar.gz from cross-built binaries + packaging/linux templates.
# Usage: ./scripts/package_linux_release.sh --version v0.1.0 --armv7 path/to/beetle --riscv64 path/to/beetle [--output-dir dist]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VERSION=""
ARMV7_BIN=""
RISCV64_BIN=""
OUTPUT_DIR="$REPO_ROOT/dist"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) VERSION="${2:-}"; shift 2 ;;
    --armv7) ARMV7_BIN="${2:-}"; shift 2 ;;
    --riscv64) RISCV64_BIN="${2:-}"; shift 2 ;;
    --output-dir) OUTPUT_DIR="${2:-}"; shift 2 ;;
    -h|--help)
      echo "Usage: $0 --version vX.Y.Z --armv7 PATH --riscv64 PATH [--output-dir DIR]"
      exit 0
      ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

if [[ -z "$VERSION" || -z "$ARMV7_BIN" || -z "$RISCV64_BIN" ]]; then
  echo "Required: --version, --armv7, --riscv64" >&2
  exit 1
fi
if [[ ! "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+ ]]; then
  echo "VERSION must look like v0.1.0, got: $VERSION" >&2
  exit 1
fi
if [[ ! -f "$ARMV7_BIN" || ! -f "$RISCV64_BIN" ]]; then
  echo "Binary not found (armv7 or riscv64)" >&2
  exit 1
fi

PKG_ROOT="$REPO_ROOT/packaging/linux"
for f in beetle.service beetle.init hardware.json.example README.txt; do
  if [[ ! -f "$PKG_ROOT/$f" ]]; then
    echo "Missing packaging template: $PKG_ROOT/$f" >&2
    exit 1
  fi
done

mkdir -p "$OUTPUT_DIR"

make_tarball() {
  local triple="$1"
  local bin_path="$2"
  local name="beetle-${VERSION}-linux-${triple}-musl"
  local stag_dir
  stag_dir="$(mktemp -d "${TMPDIR:-/tmp}/beetle-pkg-${triple}.XXXXXX")"
  mkdir -p "$stag_dir/$name"
  cp "$bin_path" "$stag_dir/$name/beetle"
  chmod 755 "$stag_dir/$name/beetle"
  cp "$PKG_ROOT/beetle.service" "$PKG_ROOT/beetle.init" "$PKG_ROOT/hardware.json.example" "$PKG_ROOT/README.txt" "$stag_dir/$name/"
  chmod 755 "$stag_dir/$name/beetle.init"
  local out="$OUTPUT_DIR/${name}.tar.gz"
  (cd "$stag_dir" && tar -czf "$out" "$name")
  rm -rf "$stag_dir"
  echo "Wrote $out"
}

make_tarball armv7 "$ARMV7_BIN"
make_tarball riscv64 "$RISCV64_BIN"
