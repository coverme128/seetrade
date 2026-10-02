# Codex Handoff v1.0.3

## What Codex Should Build First

Do not start with UI polish or AUTO.

Start with:

1. Exchange market-data capture
2. deterministic local book
3. replay
4. feature calculation
5. 1s AlphaEvaluation + Prediction Episode aggregation
6. deterministic prediction outcome
7. OBSERVE Card

Then proceed through MASTER-CONTRACT order. Implement `AutoTradeAdmission` before enabling AUTO live.

## Implementation Constraints

- single-user local/personal product
- no SaaS abstractions
- no multitenancy
- no billing
- no organization model
- no RBAC beyond local app safety if needed
- no LLM in hot path
- no queue in hot path
- no duplicate paper/live business logic

## Definition of Done

Codex must return for each phase:

- code changes
- tests added
- commands to run
- actual test results
- known gaps
- gate status PASS/FAIL
- screenshots only where UI is relevant


## v1.0.2 Prediction-Episode Baseline

Do not model each 1-second evaluation as a persisted Prediction. Implement a formal `PredictionEpisode` aggregation boundary before UI/history/learning.

The same Episode must be usable by OBSERVE, PAPER, CONFIRM and AUTO. Trading mode changes execution authority, not the prediction truth model.


## v1.0.3 Mandatory AUTO Correction

Do not map `TradeQualification=QUALIFIED` directly to a live AUTO order. Implement `AutoTradeAdmissionDecision` as a separate deterministic contract.

A Qualified Episode may remain visible/learnable but be `SKIPPED` for AUTO due to stricter edge threshold, Model Trust, execution quality, risk budget, competing opportunities or re-entry/churn protection.
