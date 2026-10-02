# Codex Implementation Plan v1.0.3

## P0 / Phase 1 — Market Data + Replay

- Binance + OKX WebSocket
- BTC + ETH
- BBO / L2 / trades/aggressor / mark/index
- OI/liquidation only if timestamp/reliability acceptable
- local order-book sequence validation and recovery
- Parquet recorder
- deterministic replay

**PASS**: recorded session can replay and reproduce the same canonical book checkpoints.

## P0 / Phase 2 — Feature + Triple Barrier Outcome

- mandatory first-wave features
- cross-venue features
- Triple Barrier / First-Hit labeler
- MFE / MAE
- Prediction Ground Truth closer

**PASS**: labels and Outcome are deterministic under replay.

## P0 / Phase 3 — Prediction + OBSERVE

- deterministic score baseline
- logistic regression baseline
- probability calibration
- 1s AlphaEvaluation loop
- Prediction Episode aggregator with hysteresis/debounce
- Prediction lifecycle
- Core Recommendation Card
- WHY NOW drivers
- Result Card

**PASS**: user can continuously watch “未来 X 秒/分钟，+Y% before -Z%”，不交易也能自动得到 Outcome；同一连续机会在 60 秒内的 60 次 Evaluation 不得默认产生 60 张正式 Prediction Card。

## P0 / Phase 4 — Trade Qualification

- Probability Edge
- Calibration
- Model Trust
- Cost-adjusted Net EV
- liquidity / spread / volatility / slippage readiness
- model regime / OOD
- NOT QUALIFIED reasons

**PASS**: high probability with negative Net EV or LOW Model Trust is rejected.

## P1 / Phase 5 — Model Health

- calibration buckets
- recent vs baseline
- Prediction Drift
- EV Drift
- Model Trust state machine
- Retrain Recommended

**PASS**: model can be downgraded without changing model weights; LOW Trust can block new live trades.

## P1 / Phase 6 — Cost Model + PAPER

- fee model
- spread cost
- slippage model
- adverse selection estimate
- PaperExecutionAdapter
- expected vs realized paper cost/P&L

**PASS**: paper trade economics is auditable from signal to result.

## P1 / Phase 7 — Position / Leverage / Exit

- risk budget
- target notional
- effective leverage
- add / hold / reduce / exit
- hysteresis / deadband
- no martingale
- de-leverage based on fresh Gate/risk recalculation

**PASS**: loss by itself cannot cause leverage increase; negative/invalid Edge can reduce/exit.

## P1 / Phase 8 — Auto Trade Admission

- separate QUALIFIED from AUTO execution eligibility
- stricter AUTO Net EV threshold
- latest-evaluation recheck before order
- deterministic concurrent-candidate priority tuple
- risk-budget awareness
- re-entry cooldown / churn guard
- ADMITTED / SKIPPED reason codes

**PASS**: AUTO does not execute every Qualified Episode; a Qualified opportunity can be deterministically skipped and audited without changing its Prediction Outcome.

## P2 / Phase 9 — CONFIRM Small Live

- live exchange execution adapter
- Execute / Skip
- tiny capital cap
- execution telemetry
- kill switch

**PASS**: every live order maps to Prediction → Qualification → TradePlan → Risk decision.

## P2 / Phase 10 — AUTO

- explicit auto permission
- exact same Risk Gate
- auto entry/add/reduce/exit
- stale feed / book gap / exchange degraded circuit breakers

**PASS**: AUTO cannot bypass Risk Gate or Model Trust restrictions.

## P3 / Phase 11 — Controlled Adaptation

- scheduled/manual retraining job
- Challenger model
- walk-forward/OOS evaluation
- Shadow prediction
- promotion report

**PASS**: production Champion changes only through explicit promotion gate.

## P3 / Phase 12 — AI Assist Optional

Only after the deterministic/quant core is stable.

AI may produce:

- WHY NOW explanation
- prediction/trade review
- model-drift summary
- research summary

AI may not change core trading outputs.
