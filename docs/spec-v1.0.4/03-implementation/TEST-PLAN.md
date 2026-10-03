# Test Plan v1.0.4

## Unit

- percentile / robust-z normalization
- each trigger family NORMAL/ABNORMAL/EXTREME
- composite trigger rule
- debounce/deduplication
- runtime state transitions
- protection priority

## Replay Golden

Must prove:

1. quiet market segment produces few/no Full Alpha Evaluations while IDLE;
2. known order-flow burst triggers immediate evaluation;
3. one EXTREME trigger works alone;
4. two ABNORMAL families trigger together;
5. one ABNORMAL alone does not trigger;
6. trigger burst does not create duplicate Prediction Episodes;
7. ACTIVE Episode retains ~1s re-evaluation;
8. fixed-step reference and event-driven mode produce comparable outcomes for known fixtures;
9. missed-opportunity report is deterministic.

## Risk/Data Failure Injection

- spread spike
- depth collapse
- slippage spike
- volatility spike
- stale WebSocket
- sequence gap
- invalid book snapshot
- exchange disconnect
- timestamp skew

Expected:
- protection path activates immediately
- new entry fails closed when data invalid
- position protection is not delayed behind unrelated account-query work

## Regression

All v1.0.3 invariants must remain green, especially:

- Trigger != Episode
- Episode != Evaluation
- QUALIFIED != AUTO EXECUTE
- ADMISSION required before AUTO entry
- skipped Episode still closes to Outcome
- prediction/trade outcome separation
