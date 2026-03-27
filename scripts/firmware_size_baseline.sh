#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

TARGET_TRIPLE="${TARGET_TRIPLE:-xtensa-esp32s3-espidf}"
PROFILE_NAME="${PROFILE_NAME:-release}"
OUT_DIR="${OUT_DIR:-target/size-baseline}"
mkdir -p "${OUT_DIR}"

TS="$(date +"%Y%m%d-%H%M%S")"
BASE="${OUT_DIR}/${TS}"

echo "[baseline] profile=${PROFILE_NAME} target=${TARGET_TRIPLE}"
echo "[baseline] output prefix: ${BASE}"

{
  echo "timestamp=${TS}"
  echo "profile=${PROFILE_NAME}"
  echo "target=${TARGET_TRIPLE}"
  echo "features=${FEATURES:-default}"
} > "${BASE}.meta"

echo "[baseline] running cargo llvm-lines ..."
if [[ -n "${FEATURES:-}" ]]; then
  cargo llvm-lines --"${PROFILE_NAME}" --bin beetle --target "${TARGET_TRIPLE}" \
    --no-default-features --features "${FEATURES}" > "${BASE}.llvm-lines.txt"
else
  cargo llvm-lines --"${PROFILE_NAME}" --bin beetle --target "${TARGET_TRIPLE}" \
    > "${BASE}.llvm-lines.txt"
fi

{
  echo "=== llvm-lines top 40 ==="
  sed -n '1,40p' "${BASE}.llvm-lines.txt"
} > "${BASE}.summary.txt"

if command -v idf.py >/dev/null 2>&1; then
  echo "[baseline] running idf.py size-components ..."
  idf.py size-components > "${BASE}.idf-size-components.txt"
  {
    echo ""
    echo "=== idf.py size-components (top 80 lines) ==="
    sed -n '1,80p' "${BASE}.idf-size-components.txt"
  } >> "${BASE}.summary.txt"
else
  echo "[baseline] idf.py not found; skipped idf.py size-components" | tee -a "${BASE}.summary.txt"
fi

echo "[baseline] done. summary: ${BASE}.summary.txt"
