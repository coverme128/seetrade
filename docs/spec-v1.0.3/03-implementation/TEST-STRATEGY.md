# Test Strategy v1.0.3

## Unit

- order book apply delta
- sequence gap detection
- feature math
- label first-hit ordering
- EV calculation
- position sizing
- leverage caps
- gate logic
- no-martingale rule

## Replay Golden Tests

固定一组 market recordings，验证：

- same FeatureSnapshot
- same Prediction
- same Qualification
- same TradePlan

## Simulation

- spread widening
- WS gap
- stale data
- exchange disconnect
- slippage spike
- volatility spike
- partial fill
- API reject
- liquidation cascade

## Live Safety

- sandbox/testnet where applicable
- tiny notional live test
- max order size hardcoded env cap for first live phase
- kill switch drill


## Prediction Episode Tests

Must cover:

1. `1000ms` evaluation loop can run without creating one Prediction record per tick.
2. Small probability oscillation inside hysteresis band updates the same Episode.
3. WATCH → EMERGING → QUALIFIED remains one Episode when it is one continuous opportunity.
4. TARGET/STOP/TIMEOUT closes Episode deterministically under replay.
5. OBSERVE Episode produces Outcome with zero Execution records.
6. Prediction Outcome and Trade Outcome can differ and remain separately queryable.
7. Restart/replay does not duplicate already-closed Episode results.
8. DATA_INVALIDATED is excluded from win/loss calibration denominator according to explicit evaluation policy.


## Auto Trade Admission Tests v1.0.3

Must prove:

- QUALIFIED does not imply AUTO execution.
- negative/insufficient AUTO Net EV => SKIPPED even if normal Trade Qualification is PASS.
- stale latest Evaluation => SKIPPED.
- LOW/insufficient Model Trust => SKIPPED.
- execution health fail => SKIPPED.
- risk budget unavailable => SKIPPED.
- simultaneous candidates with limited budget select deterministically from the configured priority tuple.
- re-entry cooldown blocks churn.
- `max_daily_trades` cannot lower thresholds or force a trade.
- any AUTO live entry without ADMITTED record fails closed.
