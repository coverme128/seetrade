# Codex Handoff — v1.1

## Instruction

Implement this bundle only after the v1.0.4 Runtime Trigger Upgrade is present. Treat v1.0.4 as the production runtime contract and do not restore the old all-asset fixed 1s Full Alpha loop.

## Required sequence

### Batch 01 — Asset Domain
- schema
- asset/instrument identity
- lifecycle state machine
- AssetProfile + AssetTriggerProfile contracts

### Batch 02 — Scanner + Manual Add
- market enumeration
- lightweight metrics
- Research Priority
- scheduled run + Scan Now
- targeted manual add using same path

### Batch 03 — Warmup
- candidate subscription pool
- Parquet/raw storage
- data-health metrics
- COLLECTING → CALIBRATING support

### Batch 04 — Per-Asset Integration
- normalization/profile lookup
- per-asset trigger-profile lookup/calibration
- per-asset model/calibration/cost/risk references
- OBSERVE predictions/outcomes

### Batch 05 — Opportunity Radar
- aggregate triggered/active recommendations; Eligible IDLE assets are not opportunities by default
- drill-down to inherited detailed card

### Batch 06 — Portfolio Exposure Gate
- shadow first
- enforcement before multi-coin live

### Batch 07 — Multi-Asset Validation
- OBSERVE and PAPER
- compare expected vs realized costs/EV
- validate cross-asset concentration guard

### Batch 08 — Controlled Live
- only explicitly eligible assets
- preserve kill switch
- no automatic all-coin AUTO rollout

## Do not implement

- AI coin picker
- online self-learning
- AutoML
- model auto-promotion
- scanner-to-order shortcut
- per-coin strategy code forks
- heavy distributed infra without measured need

## Mandatory evidence in Codex completion report

- migration output
- schema/tests
- scanner sample run
- one manual-add flow
- one rejected asset with reason
- one OBSERVE asset Prediction→Outcome
- one Portfolio Gate block/reduce example
- BTC/ETH regression test results
- rollback test result


## Baseline v1.0.4 Enforcement

Do not connect `QUALIFIED` or Scanner ranking directly to multi-coin AUTO execution. Reuse the v1.0.4 Auto Trade Admission contract, then apply the v1.1 Portfolio Exposure Gate before Risk/Execution.


## Mandatory v1.0.4 Runtime Evidence

Codex completion report must additionally include:

- one Eligible IDLE asset showing continuous Market Pulse without fixed 1s Full Alpha;
- one Order Flow/Aggressive/Book/Velocity trigger launching Full Evaluation;
- one trigger that produces `NO EDGE` and no Card/order;
- two assets showing different normalized trigger thresholds/severity;
- one ACTIVE Episode retaining ~1s evaluation;
- one Risk/Data Trigger forcing immediate protection re-evaluation;
- replay comparison against fixed-step reference evaluation.
