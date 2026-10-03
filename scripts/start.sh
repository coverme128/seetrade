#!/usr/bin/env bash
# Optimized single-machine runtime; dev.sh remains available for editing.
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh
[ -d web/node_modules ] || (cd web && npm ci)
cargo build --locked --release
(cd web && npm run build)
target/release/micro-alpha-engine "$@" &
engine_pid=$!
(cd web && exec node node_modules/next/dist/bin/next start --hostname 127.0.0.1) &
web_pid=$!
cleanup() {
  kill -INT "$engine_pid" 2>/dev/null || true
  kill -TERM "$web_pid" 2>/dev/null || true
  wait "$engine_pid" 2>/dev/null || true
}
trap cleanup EXIT INT TERM
echo 'Micro Alpha: http://127.0.0.1:3000 — Ctrl+C 停止'
while kill -0 "$engine_pid" 2>/dev/null && kill -0 "$web_pid" 2>/dev/null; do sleep 1; done
exit 1
