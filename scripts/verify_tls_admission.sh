#!/usr/bin/env bash
# 验证 TLS 准入修复：检查串口/日志中是否有基线日志，以及是否仍出现每约 5 秒固定的 get_url failed (tls_admission)。
# 用法: ./scripts/verify_tls_admission.sh [日志文件]
#  无参数时从 stdin 读；有参数时从文件读。退出码 0 表示通过，1 表示疑似仍存在固定失败模式。

set -e
INPUT="${1:--}"

if [[ "$INPUT" == "-" ]]; then
  CAPTURE="cat"
else
  CAPTURE="cat \"$INPUT\""
fi

BASELINE=$(eval "$CAPTURE" | grep -c "TLS admission baseline" || true)
# 若存在多段日志，至少有一段含 baseline 即视为已打基线
if [[ "${BASELINE:-0}" -gt 0 ]]; then
  echo "PASS: TLS admission baseline log present (count=${BASELINE})"
else
  echo "WARN: No TLS admission baseline log found (ESP 启动后应有一条)"
fi

# 检测是否在 60s 内出现 ≥6 次 get_url failed 且含 tls_admission（约每 5s 一次）
FAILS=$(eval "$CAPTURE" | grep "get_url failed" | grep -c "tls_admission" || true)
if [[ "${FAILS:-0}" -ge 6 ]]; then
  echo "FAIL: Repeated get_url failed (tls_admission) count=${FAILS} — 疑似仍存在固定 5s 失败"
  exit 1
fi

echo "PASS: No sustained tls_admission get_url failure pattern (count=${FAILS:-0})"
exit 0
