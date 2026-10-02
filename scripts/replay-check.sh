#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh
cargo build --locked
fixture_result=$(cargo run --locked --example fixture -- data/demo-fixture.jsonl 240)
result=$(target/debug/micro-alpha-engine --replay data/demo-fixture.jsonl)
python3 - "$result" "$fixture_result" <<'PY'
import json,sys
from pathlib import Path
result=json.loads(sys.argv[1])
fixture=json.loads(sys.argv[2])
assert result['state_sha256']==fixture['state_sha256'],(result,fixture)
assert result['outcomes']==fixture['outcomes'] and result['trades']==fixture['trades'],(result,fixture)
expected=Path('fixtures/demo-240.sha256').read_text().strip()
assert result['state_sha256']==expected,result
assert result['outcomes']==7,result
print('PASS: canonical books, features, predictions, qualifications and outcomes match the frozen replay digest.')
PY
