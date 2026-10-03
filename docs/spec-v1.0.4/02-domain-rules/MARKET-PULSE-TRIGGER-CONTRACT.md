# Market Pulse & Trigger Contract v1.0.4

## 1. Purpose

Market Pulse is a cheap, continuously updated detector of meaningful market-state change. It exists only to decide whether to run Full Alpha Evaluation while an asset is IDLE.

It does not output LONG/SHORT, probability, Net EV, Trade Qualification, position size, leverage or an order.

## 2. Runtime States

`IDLE -> CANDIDATE -> ACTIVE -> IDLE`

With live position:

`ACTIVE -> IN_POSITION -> ACTIVE/IDLE`

- `IDLE`: lightweight pulse only
- `CANDIDATE`: trigger occurred; full evaluation active
- `ACTIVE`: formal Prediction Episode exists
- `IN_POSITION`: live/paper position exists; full evaluation + protection triggers

## 3. Alpha Trigger Families

### A. ORDER_FLOW_CHANGE
Detect a meaningful change in signed order-flow imbalance or persistence.

Candidate lightweight inputs:
- rolling OFI
- OFI delta
- same-direction persistence
- flow reversal magnitude

### B. AGGRESSIVE_TRADE_BURST
Detect unusual aggressor activity.

Candidate inputs:
- aggressive buy/sell imbalance
- trade arrival rate
- notional burst
- consecutive same-side trades

### C. BOOK_IMBALANCE_DEPTH
Detect unusual L2 pressure or rapid depth depletion.

Candidate inputs:
- top-N book imbalance
- bid/ask depth delta
- same-side depletion rate
- microprice displacement may be used as context

### D. PRICE_VELOCITY
Detect abnormal short-horizon price motion.

Candidate inputs:
- short return
- return velocity
- acceleration
- micro-range expansion

## 4. Normalization

Trigger thresholds must be relative to the asset's recent behavior, not universal absolute values.

Allowed first-wave normalization:
- rolling percentile bands; or
- robust z-score / median absolute deviation.

The implementation must version the normalization window and threshold config.

Do not hardcode “+0.1% always triggers” across assets.

## 5. Severity

Each Alpha Trigger family produces:

- `NORMAL`
- `ABNORMAL`
- `EXTREME`

V1.0.4 does not require more severity levels.

## 6. Combination Rule

Trigger Full Alpha Evaluation when:

- at least one family is `EXTREME`; OR
- at least two distinct families are `ABNORMAL` in the same trigger window.

The exact percentile/z-score boundaries are mutable configuration and must be calibrated from replay; they are not frozen product constants.

## 7. Trigger Event

`MarketPulseTriggerEvent` fields:

- trigger_id
- symbol
- exchange_scope
- occurred_at_exchange_ts
- received_at_local_ts
- trigger_families[]
- severity_by_family
- normalized_metric_snapshot
- normalization_config_version
- runtime_state_before
- action: `FULL_ALPHA_EVALUATION`

## 8. Debounce / Deduplication

A burst of raw market events must not create a trigger storm.

Requirements:
- merge materially identical triggers inside a configurable debounce window
- stronger severity may upgrade the existing trigger context
- same continuous opportunity should remain within the same Candidate/Prediction Episode path
- a trigger alone must never create a new Prediction Episode if Episode creation criteria are not met

## 9. Candidate Lifecycle

After Alpha Trigger:

1. run Full Alpha Evaluation immediately;
2. if no meaningful edge, remain candidate briefly or return IDLE according to config;
3. while CANDIDATE, evaluate every 1–2s or on stronger Trigger;
4. if formal Prediction Episode is created, move ACTIVE;
5. if candidate expires without an Episode, return IDLE.

## 10. Invariants

- Trigger != Prediction
- Trigger != Qualification
- Trigger != Trade
- Trigger != Auto Admission
- Trigger thresholds cannot bypass Net EV/cost/risk gates
