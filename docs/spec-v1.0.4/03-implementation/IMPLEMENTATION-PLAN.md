# Implementation Plan v1.0.4

## Batch 01 — Market Pulse + Runtime State

Implement:
- incremental pulse snapshot
- asset-normalized rolling baseline
- IDLE/CANDIDATE/ACTIVE/IN_POSITION runtime state
- remove unconditional IDLE 1s Full Alpha Evaluation

Acceptance:
- IDLE asset can run for minutes with live market updates but zero Full Alpha Evaluations when no Trigger fires
- raw book/trade capture remains continuous

## Batch 02 — Four P0 Alpha Triggers

Implement:
- ORDER_FLOW_CHANGE
- AGGRESSIVE_TRADE_BURST
- BOOK_IMBALANCE_DEPTH
- PRICE_VELOCITY
- severity NORMAL/ABNORMAL/EXTREME
- `ANY EXTREME OR 2+ ABNORMAL`
- debounce/deduplication

Acceptance:
- synthetic/replay trigger fixtures deterministically fire or do not fire
- Trigger immediately starts Full Alpha Evaluation
- Trigger never creates an order

## Batch 03 — Candidate / Active Cadence

Implement:
- immediate Full Evaluation on trigger
- Candidate 1–2s re-evaluation
- Active Episode ~1s re-evaluation
- Position ~1s re-evaluation
- return to IDLE after candidate/episode closure

Acceptance:
- same opportunity remains one Prediction Episode
- Active behavior remains v1.0.3 compatible

## Batch 04 — Protection Triggers

Implement:
- Risk Trigger
- Data Integrity Trigger
- event-driven immediate protection path
- fail-closed new-entry block on stale/invalid data

Acceptance:
- risk/data fixture can interrupt normal cadence immediately
- existing Risk Engine remains final action authority

## Batch 05 — Trigger Analytics + Replay Comparison

Implement:
- trigger funnel metrics
- origin trigger link on Episode
- fixed-step replay reference mode
- missed-opportunity comparison

Acceptance:
- report can compare event-driven production policy with 1s reference replay without enabling full-scan production runtime

## Batch 06 — Regression / Live Closure

Re-run all v1.0.3 tests plus:
- Prediction Episode integrity
- Auto Trade Admission
- live execution protection
- Model Trust / Execution Drift feedback

No v1.1 Coin Scanner work in this package.
