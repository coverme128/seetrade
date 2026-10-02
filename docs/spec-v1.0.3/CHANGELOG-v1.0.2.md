# CHANGELOG — v1.0.2

## Purpose

Core correction before v1.1. No new product surface or SaaS scope.

## Added / Frozen

- 1000ms default Alpha / Trade Qualification evaluation cadence.
- Explicit separation between ephemeral `AlphaEvaluation` and persistent `PredictionEpisode`.
- Episode aggregation, hysteresis/debounce and stable Card behavior are domain/implementation requirements.
- Every formal Episode automatically resolves to objective market Outcome in OBSERVE, PAPER, CONFIRM or AUTO.
- Prediction Outcome and Trade Outcome are separate truth domains.
- Calibration / Model Trust / Drift use closed Episodes, not each one-second evaluation tick.
- Runtime and daily metrics explicitly separate evaluations, episodes, qualified episodes and trades.

## Not Added

- no new exchange
- no new coin universe
- no AI prediction
- no online learning
- no AutoML
- no automatic model promotion
- no SaaS features

## Compatibility

Supersedes v1.0.1 as the implementation baseline. Existing core Recommendation, Trade Qualification, Risk, Cost, execution mode and AI boundaries remain unchanged.
