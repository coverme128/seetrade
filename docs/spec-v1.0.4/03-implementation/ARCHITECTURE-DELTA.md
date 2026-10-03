# Architecture Delta v1.0.4

## Before

`WebSocket -> Book/Features -> 1s Full AlphaEvaluation for every asset -> Episode Aggregator -> Qualification`

## After

```text
Exchange WebSocket
      ↓
Canonical Book / Trades
      ↓
Incremental Market Pulse
      │
      ├─ IDLE: cheap pulse only
      │       ↓ meaningful trigger
      │   Full Alpha Evaluation
      │       ↓
      │   CANDIDATE
      │       ↓
      │   Prediction Episode / ACTIVE
      │
      └─ Protection Trigger
              ↓
         immediate Risk/Data response

ACTIVE / IN_POSITION
      ↓
~1s Full Alpha Evaluation
      ↓
Qualification / Admission / Risk / Execution
```

## Modules

Add small modules to the existing engine; do not create new microservices:

- `market_pulse`
- `trigger_policy`
- `trigger_analytics`

Reuse existing:
- market/book state
- feature calculations where incremental forms already exist
- alpha model
- Prediction Episode
- qualification
- auto admission
- risk / position / execution

## Hot Path

No queue, database round-trip, LLM, or external service is required to decide a Trigger.

Market Pulse must operate in-memory from live canonical market state.

## Storage

Current personal architecture (`SQLite + local files/Parquet`) remains acceptable.

Persist compact Trigger events and analytics references; do not persist every raw pulse tick as a new business object when raw market data is already recorded.
