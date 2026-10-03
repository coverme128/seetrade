# Acceptance Gates v1.0.4

## G1 — IDLE_EFFICIENCY_PASS

- continuous market data remains healthy
- IDLE assets do not require full 1s Alpha Evaluation
- Market Pulse computation is incremental/lightweight

## G2 — TRIGGER_DETERMINISM_PASS

- all 4 Alpha Trigger families have deterministic fixtures
- one EXTREME or two ABNORMAL triggers Full Evaluation
- no Trigger directly creates TradePlan/order

## G3 — EPISODE_COMPATIBILITY_PASS

- same continuous opportunity remains one Prediction Episode
- skipped/non-traded Episode still receives Ground Truth Outcome
- Calibration/Model Trust continue using closed Episodes

## G4 — PROTECTION_PASS

- Risk/Data triggers can preempt normal evaluation cadence
- stale/corrupt data blocks new trades fail-closed
- open-position protection is independently executable

## G5 — AUTO_REGRESSION_PASS

- v1.0.3 Trade Qualification remains unchanged
- Auto Admission remains required
- Risk Gate remains required
- no churn regression

## G6 — TRIGGER_ANALYTICS_PASS

- trigger funnel report exists
- Episode can be linked back to origin trigger
- fixed-step replay comparison identifies potential missed opportunities

## G7 — PRODUCTION_RUNTIME_READY

All previous gates PASS, continuous runtime has no trigger storm, no runaway evaluation loop, no duplicate Episode, and resource use remains stable.
