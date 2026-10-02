#!/usr/bin/env python3
"""Streaming recording audit. Flags are evidence, never repairs missing observations."""
import argparse
import collections
import json
import sqlite3
import contextlib
from pathlib import Path


def audit(path, stale_ms=3000, db_path=None):
    streams = collections.defaultdict(lambda: dict(events=0, gaps=0, stale=0, backwards=0, max_delay_ms=0, sequence=None, last_ts=None))
    counts = collections.Counter()
    invalid = collections.Counter()
    pending = {}
    outcomes = 0
    malformed = []
    with Path(path).open() as f:
        for line_no, line in enumerate(f, 1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
                kind = row.get('kind', row.get('type'))
                # Journal records use tagged type; derived records use kind.
                event = row.get('event')
                if event:
                    name = event['event_type']
                    counts[name] += 1
                    if name == 'clock':
                        continue
                    key = f"{event['exchange']}:{event['symbol']}:{name}"
                    s = streams[key]
                    ts, received = event['exchange_ts'], event['receive_ts']
                    s['events'] += 1
                    s['max_delay_ms'] = max(s['max_delay_ms'], abs(received-ts))
                    s['stale'] += abs(received-ts) > stale_ms
                    s['backwards'] += s['last_ts'] is not None and ts < s['last_ts']
                    s['last_ts'] = ts
                    # Snapshot and delta must share one book sequence.
                    if name in ('snapshot', 'delta'):
                        book = streams[f"{event['exchange']}:{event['symbol']}:book"]
                        if name == 'delta' and book['sequence'] is not None:
                            seq = book['sequence']
                            if event['exchange'] == 'binance' and not book.get('bridged', False):
                                gap = event['first'] > seq or event['sequence_id'] < seq
                            else:
                                gap = event['previous'] != seq
                            book['gaps'] += gap
                        book['sequence'] = event['sequence_id']
                        book['bridged'] = name == 'delta' or event['exchange'] != 'binance'
                p = row.get('payload', {})
                if kind == 'prediction':
                    pending[p['prediction_id']] = p['ts'] + p['horizon']*1000
                elif kind == 'outcome':
                    outcomes += 1
                    pending.pop(p['prediction']['prediction_id'], None)
                    if not p['learning_eligible']:
                        invalid[p.get('prediction_error', 'unknown')] += 1
            except (ValueError, KeyError, TypeError) as e:
                malformed.append({'line': line_no, 'error': str(e)})
    if db_path:
        with contextlib.closing(sqlite3.connect(f'file:{Path(db_path).resolve()}?mode=ro', uri=True)) as db:
            for (encoded,) in db.execute("SELECT payload FROM records WHERE kind IN ('prediction','outcome') ORDER BY id"):
                record=json.loads(encoded);p=record['payload']
                if record['kind']=='prediction': pending[p['prediction_id']]=p['ts']+p['horizon']*1000
                else:
                    outcomes+=1;pending.pop(p['prediction']['prediction_id'],None)
                    if not p['learning_eligible']: invalid[p.get('prediction_error','unknown')]+=1
    problems = sum(s['gaps'] + s['stale'] + s['backwards'] for s in streams.values())
    return dict(schema='dataset-quality-v1', path=str(path), events=dict(counts), streams=dict(streams),
                outcomes=outcomes, invalid_outcomes=dict(invalid), unresolved_predictions=len(pending),
                malformed=malformed[:100], malformed_count=len(malformed),
                status='PASS' if counts and not problems and not malformed else 'REVIEW_REQUIRED',
                limitations='Unresolved forecasts at recording end are right-censored; do not relabel as timeout. Run on journal for continuity and derived records for label quality.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('recording', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--db', type=Path, help='Optional matching SQLite database for label quality')
    parser.add_argument('--stale-ms', type=int, default=3000)
    args = parser.parse_args()
    text = json.dumps(audit(args.recording, args.stale_ms, args.db), ensure_ascii=False, indent=2)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text+'\n')
    else:
        print(text)
