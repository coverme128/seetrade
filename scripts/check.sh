#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --locked --all-targets -- -D warnings
(cd web && npm run format:check && npm run typecheck && npm run build)
./scripts/replay-check.sh
if [ -x .venv/bin/python ]; then
  .venv/bin/python -m unittest discover -s tests -p 'test_*.py'
else
  python3 -m unittest discover -s tests -p 'test_*.py'
fi
