# Live Gates v1.0.3

## Gate A — OBSERVE_READY

Required:

- market data continuity stable
- replay deterministic
- Prediction/Outcome loop works
- calibration report generated
- no money trading

## Gate B — MODEL_HEALTH_READY

Required:

- recent vs baseline calibration exists
- Model Trust state machine works
- at least Prediction Drift is operational
- LOW Trust can prevent progression to live qualification
- Retrain Recommended can be produced without mutating Champion

## Gate C — PAPER_READY

Required:

- cost model works
- paper fills work
- expected vs realized paper P&L tracked
- EV Drift and basic Execution Drift available

## Gate D — CONFIRM_LIVE_READY

Required:

- hard risk caps tested
- kill switch tested
- tiny live capital only
- every execution auditable
- Model Trust is not LOW
- no unresolved data-integrity issue

## Gate E — AUTO_LIVE_READY

Only after:

- sufficient sample size defined by research config
- cost-adjusted paper/small-live evidence positive over multiple windows
- calibration acceptable
- Model Trust NORMAL/HIGH
- drawdown/tail loss acceptable
- no unresolved market-data integrity issues
- execution quality within limits
- AUTO-specific add/reduce/exit tests pass
- Auto Trade Admission tested in shadow/PAPER mode
- Qualified opportunities can be SKIPPED without being reclassified as NOT QUALIFIED
- every AUTO entry has ADMITTED audit record
- re-entry/churn guard works
- max daily trade count is a cap, not an execution target

AUTO defaults OFF until explicitly enabled.

## Gate F — RETRAIN_PROMOTION_READY

A Challenger may replace Champion only after:

- walk-forward/OOS validation
- shadow comparison
- calibration not worse
- cost-adjusted EV not worse
- drawdown/tail risk within constraints
- sufficient observations

V1 does not require fully automatic promotion.


## Prediction Episode Integrity Gate

Before PAPER/Live:

- 1s evaluation must not inflate Prediction sample count.
- every formal Episode must reach exactly one terminal outcome or explicit DATA_INVALIDATED state.
- OBSERVE outcome closure must work with no order/execution dependency.
- calibration must use closed Episodes, not raw Evaluation ticks.
- replay must reproduce Episode boundaries and terminal outcome under the same config/model artifact.
