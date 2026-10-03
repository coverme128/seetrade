# Protection Trigger Contract v1.0.4

## 1. Purpose

Protection triggers exist to protect capital and system integrity. They are independent from Alpha opportunity triggers.

Two categories:

- `RISK_TRIGGER`
- `DATA_TRIGGER`

## 2. Risk Trigger — P0

When a position exists, immediately request risk re-evaluation on material deterioration such as:

- spread expansion beyond configured safety band
- material depth collapse / liquidity deterioration
- expected slippage spike
- realized-volatility spike / fast adverse move
- exchange execution-health degradation

The existing Risk/Position/Exit engines decide HOLD / REDUCE / EXIT. The trigger itself must not invent a new exit rule.

## 3. Data Integrity Trigger — P0

Trigger protection when market/execution state becomes unreliable, including:

- stale WebSocket / stale market snapshot
- local order-book sequence gap not yet recovered
- invalid/corrupt book state
- exchange stream degraded/disconnected
- material exchange/local timestamp skew beyond configured tolerance

Required default behavior:

- block new entries fail-closed
- invalidate or pause new Alpha/Admission decisions that depend on stale state
- if a position exists, invoke the existing configured protective risk path

## 4. Priority

Protection triggers have higher runtime priority than normal Alpha evaluation cadence.

They may force immediate evaluation without waiting for:
- Candidate 1–2s cadence
- Active ~1s cadence

## 5. Audit Event

`ProtectionTriggerEvent`:

- id
- symbol
- occurred_at
- type: `RISK | DATA`
- reason_codes[]
- metric_snapshot
- position_open: boolean
- resulting_action: `BLOCK_NEW | REEVALUATE_RISK | REDUCE | EXIT | NO_ACTION`
- config_version

## 6. Invariants

- no LLM decision
- no bypass of existing Risk Engine
- data failure cannot silently continue normal AUTO entry
- ordinary account-query failure must not indefinitely suppress independent protective order/position processing
