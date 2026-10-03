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
trigger_fixture=$(cargo run --locked --example trigger_fixture -- data/trigger-fixture.jsonl)
trigger_replay=$(target/debug/micro-alpha-engine --replay data/trigger-fixture.jsonl)
comparison=$(cargo run --locked --example trigger_compare -- data/trigger-fixture.jsonl)
python3 - "$trigger_fixture" "$trigger_replay" "$comparison" <<'PY'
import json,sys
from pathlib import Path
direct,replay,comparison=map(json.loads,sys.argv[1:])
assert direct['state_sha256']==replay['state_sha256']==Path('fixtures/trigger-240.sha256').read_text().strip()
assert direct['outcomes']==replay['outcomes']==2
assert direct['trades']==replay['trades']==0
assert comparison['useful_reference_episodes']==comparison['matched_useful_episodes']==2
assert comparison['potentially_missed_episodes']==[]
assert comparison['event_driven']['episodes']==comparison['fixed_step_reference']['episodes']==2
assert comparison['event_driven']['evaluations']<comparison['fixed_step_reference']['evaluations']
print('PASS: event-driven fixture matches canonical replay; two useful reference episodes matched with fewer full evaluations.')
PY
