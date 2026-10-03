# Micro Alpha Trading — v1.0.4 Runtime Trigger Upgrade Contract

## 1. Baseline

- Base version: `micro-alpha-trading-commercial-v1.0.3`
- Upgrade type: runtime behavior + trigger domain + observability
- Scope: personal single-user product only
- This package is incremental. Do not rebuild or replace the validated v1.0.3 prediction, qualification, admission, risk, or execution contracts.

## 2. Upgrade Goal

Replace the production runtime assumption:

`all eligible assets -> full AlphaEvaluation every ~1s all day`

with:

`continuous market listening -> lightweight Market Pulse -> meaningful-change trigger -> Full Alpha Evaluation -> Prediction Episode -> Qualification -> optional execution`.

For active opportunities and open positions, retain ~1s full re-evaluation because the system must continuously re-check edge, risk, reduce, and exit conditions.

## 3. Why This Upgrade Exists

v1.0.3 correctly separated 1-second Evaluation from Prediction Episode publication, but a fixed 1-second full evaluation loop across all assets still performs many repeated calculations during unchanged market states.

The product goal is not maximum calculation count. It is:

> detect meaningful short-horizon market change, evaluate it thoroughly, publish few useful opportunities, trade selectively, verify every result.

## 4. Frozen Product Principles

The following v1.0.3 items remain unchanged:

- Recommendation-first product model
- Triple Barrier / First-Hit prediction semantics
- Prediction Episode aggregation
- Ground Truth Outcome for every formal Episode
- Trade Qualification Gate
- `QUALIFIED != AUTO EXECUTE`
- Auto Trade Admission Gate
- Cost-adjusted Net EV requirement
- Calibration / Model Trust / Drift
- OBSERVE / PAPER / CONFIRM / AUTO
- Prediction Outcome != Trade Outcome
- Risk Engine authority above Alpha Engine
- no LLM/AI in the trading hot path
- no online self-modification of the production model
- no martingale / loss chasing

## 5. New Runtime Principle

Production runtime must follow:

`Listen Continuously -> Evaluate Selectively -> Trade Rarely -> Verify Everything`.

### IDLE
- maintain live book/trade state
- compute only lightweight Market Pulse metrics incrementally
- no fixed 1s Full Alpha Evaluation requirement

### CANDIDATE
- entered when Market Pulse emits a valid Alpha Trigger
- run Full Alpha Evaluation immediately
- while candidate remains unresolved, evaluate every 1–2s or on a stronger trigger

### ACTIVE
- WATCH / EMERGING / QUALIFIED Prediction Episode exists
- Full Alpha Evaluation approximately every 1s
- continue until Episode closes or invalidates

### IN_POSITION
- Full Alpha Evaluation approximately every 1s
- Risk/Data triggers are event-driven and may force immediate re-evaluation without waiting for the next 1s cycle

## 6. V1.0.4 Trigger Scope

### Alpha Triggers — P0
1. Order Flow Change
2. Aggressive Trade Burst
3. Order Book Imbalance / Depth Depletion
4. Price Velocity / Short-term Momentum

### Protection Triggers — P0
5. Risk Trigger
6. Data Integrity Trigger

Not added as primary v1.0.4 triggers:
- OI
- liquidation flow
- advanced cross-venue lead/lag trigger
- funding trigger
- news / social / on-chain data
- AI trigger model

Existing cross-venue, volatility, liquidity and other features may still participate in Full Alpha Evaluation and Qualification.

## 7. Alpha Trigger Combination Rule

The first production rule is intentionally simple:

`ANY EXTREME signal OR at least 2 ABNORMAL signals => trigger Full Alpha Evaluation`.

A trigger means only “worth evaluating now”. It does not imply LONG/SHORT, QUALIFIED, or execution.

Thresholds must be asset-normalized and configurable. They must not use one absolute price-move threshold for all assets.

## 8. Protection Rule

Risk and data-integrity triggers are fail-safe paths.

- They do not need an Alpha Trigger.
- They must not wait for the normal Candidate/Active cadence.
- They may block new orders immediately.
- When a live position exists, they may force immediate risk re-evaluation and the existing HOLD / REDUCE / EXIT logic.

## 9. Research / Replay Exception

Offline research and deterministic replay may retain fixed-step Full Alpha Evaluation (for example every 1s) for:

- reproducible backtests
- trigger-vs-full-scan comparisons
- missed-opportunity analysis
- threshold calibration

This fixed-step mode is not a second production runtime mode and must not become a user-facing setting.

## 10. Required Analytics

Every trigger must be auditable and measurable:

- trigger type(s)
- severity
- normalized metric snapshot
- whether Full Alpha Evaluation found an actionable directional edge
- whether a Prediction Episode was created/updated
- whether it became QUALIFIED
- whether AUTO admitted/executed it
- eventual Prediction Outcome
- Paper/Live Trade Outcome when applicable

The purpose is to answer: which triggers actually lead to useful Alpha, and which create noise?

## 11. Definition of Done

v1.0.4 is complete only when:

- IDLE assets no longer require unconditional 1s Full Alpha Evaluation
- all six trigger categories are implemented and tested
- a valid Alpha Trigger starts Full Alpha Evaluation immediately
- active Episodes continue ~1s evaluation
- open positions preserve ~1s Alpha re-evaluation plus event-driven protection
- Trigger does not create an order or bypass Trade Qualification
- Trigger storms are debounced/deduplicated
- Prediction Episode semantics remain unchanged
- skipped/non-traded Episodes still close to Ground Truth Outcome
- Replay can compare event-driven runtime with fixed-step reference evaluation
- trigger analytics are queryable
- existing v1.0.3 AUTO admission/risk invariants continue to pass
