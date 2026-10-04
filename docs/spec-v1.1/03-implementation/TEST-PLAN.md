# v1.1 Test Plan

## Unit tests

- symbol normalization
- scanner metric calculations
- asset qualification component rules
- lifecycle transition legality
- debounce/persistence behavior
- AssetProfile version resolution
- portfolio exposure math
- correlation/common-market guard

## Integration tests

1. Binance/OKX metadata → normalized instruments.
2. Scanner → candidate → COLLECTING.
3. Manual add → same qualification path as scanner.
4. Feed gap → qualification FAIL/DEGRADED.
5. New asset cannot create live order.
6. OBSERVE asset can create Prediction/Outcome but not Execution.
7. ELIGIBLE asset still requires Trade Qualification.
8. Individually qualified TradePlan can be BLOCKED by Portfolio Gate.
9. BTC/ETH v1.0.4 prediction contracts remain byte/semantic compatible where applicable.
10. AI_MODE=OFF behavior remains complete.

## Replay tests

Replay the same captured dataset through v1.0.4 and v1.1 for BTC/ETH. Before Portfolio Gate, Core Recommendation and Trade Qualification outputs must remain equivalent given identical config/artifacts.

## Failure injection

- stale ticker
- L2 sequence gap
- one venue down
- partial scanner run
- symbol delisted
- sudden spread widening
- extreme volatility
- duplicate manual add
- database restart
- scanner crash mid-run

## Acceptance invariant

No code path from Scanner/Manual Add can reach exchange order placement without:

`Asset Eligible → Trade Qualified → Risk Pass → Portfolio Gate Pass → Execution Mode Permission`.


## v1.0.4 Episode Compatibility Tests

- Adding N Eligible assets must not force N × 86,400 Full Alpha Evaluations/day while those assets are IDLE; only lightweight Market Pulse scales continuously.
- A continuous active opportunity on one asset remains one Episode across repeated active-state ~1s evaluations.
- Scanner rerun does not duplicate an active Episode.
- OBSERVE assets close Episodes and generate Prediction Outcomes with zero executions.
- Calibration/Model Trust for each asset use closed Episode samples.


## v1.0.4 Auto Admission Compatibility Tests

- Multiple dynamic assets can all be Trade Qualified while only a subset is ADMITTED.
- Scanner research priority does not override AUTO Admission hard gates.
- ADMITTED candidate can still be blocked by Portfolio Exposure Gate.
- AUTO live order without inherited v1.0.4 ADMITTED record fails closed.
- Qualified-but-skipped Episodes still close to Prediction Outcomes and remain in model-health statistics.


## v1.0.4 Runtime Trigger Compatibility Tests

- Eligible IDLE asset does not run unconditional ~1s Full Alpha Evaluation.
- Market Pulse remains continuously updated from live/replayed events.
- ANY EXTREME or 2 distinct ABNORMAL trigger families launches Full Alpha Evaluation.
- Trigger without Alpha/EV does not create a formal opportunity card or order.
- CANDIDATE transitions to ACTIVE/IDLE correctly.
- ACTIVE Episode retains ~1s evaluation.
- Open position retains ~1s Alpha re-evaluation plus immediate Risk/Data protection.
- Per-asset Trigger Profile produces different normalized severities for identical raw moves when asset distributions differ.
- Replay fixed-step reference mode can measure opportunities missed by event-driven mode.
- Scanner ranking never acts as an Alpha Trigger.
