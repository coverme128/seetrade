# Architecture Delta — v1.0.4 → v1.1

## Inherited architecture

Keep the implemented personal single-machine architecture. Do not introduce infrastructure solely because the asset universe becomes dynamic.

- Rust hot path for market/book/features/trigger/alpha/qualification/admission/risk/execution
- Python for research/replay/training/calibration
- TypeScript/Next.js for UI/control
- SQLite for durable product/audit facts in the current personal deployment
- local filesystem + Parquet for raw/replay datasets
- existing cache/ephemeral mechanisms only where already useful
- no queue in the execution hot path
- no Kafka/Flink/Kubernetes/PostgreSQL/MinIO migration unless measured operating limits later justify it

## New v1.1 modules

Recommended additions inside the existing codebase:

```text
engine/
  universe/
    scanner.rs
    asset_qualification.rs
    lifecycle.rs
    profile.rs
    trigger_profile.rs
  portfolio/
    exposure_gate.rs

apps/web/
  coin-scanner/
  opportunities/
```

No new deployable microservice is required.

## Production data flow

```text
Exchange metadata + rolling lightweight stats
        ↓
Coin Scanner (5m default)
        ↓
Asset Qualification
        ↓
Lifecycle / AssetProfile / AssetTriggerProfile
        ↓
Eligible Universe Manager
        ↓
Continuous lightweight Market Pulse per eligible asset
        ↓
Meaningful Alpha Trigger
        ↓
Full Alpha Evaluation
        ↓
Prediction Episode / Trade Qualification
        ↓
Auto Trade Admission (AUTO only)
        ↓
Portfolio Exposure Gate
        ↓
Existing Risk + Execution
```

## Subscription / compute tiers

Do not subscribe to deep L2 for every listed coin immediately.

1. **Discovery tier** — exchange metadata + lightweight ticker/trade/spread/summary observations.
2. **Candidate tier** — bounded L2/trade collection for warmup, trigger-baseline calibration and qualification.
3. **Eligible IDLE tier** — maintain the v1.0.4 lightweight Market Pulse; no unconditional ~1s Full Alpha Evaluation.
4. **Triggered/Active tier** — Full Alpha Evaluation immediately; CANDIDATE 1–2s, ACTIVE ~1s.
5. **In-position tier** — ~1s Alpha re-evaluation plus immediate event-driven Risk/Data protection.

This prevents dynamic-universe growth from becoming `number_of_assets × 86,400 full evaluations/day` when markets are idle.

## Asset-aware trigger calibration

The trigger families are shared, but normalization/thresholds are asset-specific. `AssetTriggerProfile` must version the recent distribution/baselines for:

- Order Flow Change
- Aggressive Trade Burst
- Order Book Imbalance / Depth Depletion
- Price Velocity / Short-term Momentum
- Risk Trigger inputs
- Data Health thresholds

Do not use one absolute trigger threshold across BTC, ETH and altcoins.

## Evaluation / Episode boundary

For every eligible asset:

- market/book state: event-driven;
- Market Pulse: lightweight incremental/event-driven;
- IDLE: no fixed full Alpha cadence;
- CANDIDATE: immediate Full Evaluation, then about 1–2s while unresolved;
- ACTIVE Episode: about 1s Full Evaluation;
- PredictionEpisode: state-transition/event driven and stable;
- Outcome: automatically closed from real market path.

Scanner result and Market Pulse trigger never become a Prediction directly.

## AUTO path

For every dynamic asset:

`Trigger → Full Alpha Evaluation → PredictionEpisode → TradeQualification → AutoTradeAdmission → PortfolioExposure → Risk → Execution`.

No Scanner result, Research Priority, Asset Qualification, Trigger severity or Radar rank can substitute for downstream gates.

## Concurrency and supervision

Use bounded per-asset tasks/subscriptions and central supervision. A single asset feed failure must not stop all active assets. Candidate L2 slots remain bounded.

## Data health

Each asset/exchange feed maintains last event timestamp, sequence continuity where available, resync count, stale flag and dropped/out-of-order count. Data Integrity Trigger may block new trading immediately and force protection re-evaluation for open positions.
