# Baseline Update — v1.0.4

Required base: `Micro Alpha Trading v1.0.4 Runtime Trigger Upgrade` applied over the implemented Commercial Personal v1.0.3 baseline.

The functional v1.1 scope remains the controlled Dynamic Coin Universe upgrade. v1.1 MUST inherit the v1.0.4 production runtime contract:

`Continuous Market Listening → Lightweight Market Pulse → Meaningful Trigger → Full Alpha Evaluation → Prediction Episode → Trade Qualification → Auto Trade Admission → Portfolio Exposure → Risk/Execution → Outcome`.

For dynamically admitted assets:

- IDLE assets do **not** run unconditional ~1s Full Alpha Evaluation all day;
- IDLE assets maintain lightweight incremental Market Pulse state;
- a valid Alpha Trigger moves the asset to CANDIDATE and launches Full Alpha Evaluation immediately;
- CANDIDATE assets re-evaluate about every 1–2s or on stronger triggers;
- ACTIVE Prediction Episodes re-evaluate about every 1s;
- IN_POSITION assets retain about 1s Alpha re-evaluation plus immediate event-driven Risk/Data protection;
- Trigger never means LONG/SHORT, QUALIFIED, ADMITTED, or EXECUTE;
- every formal Prediction Episode still closes to objective Ground Truth Outcome;
- Research/Replay may retain fixed-step full evaluation for reproducibility and missed-opportunity comparison, but this is not a production runtime mode.

The first production trigger family remains intentionally narrow:

1. Order Flow Change
2. Aggressive Trade Burst
3. Order Book Imbalance / Depth Depletion
4. Price Velocity / Short-term Momentum
5. Risk Trigger
6. Data Integrity Trigger

Cross-venue advanced triggers, OI, liquidation-flow triggers, news/social/on-chain triggers and AI trigger models are not added by v1.1. Existing cross-venue/volatility/liquidity features may still participate in Full Alpha Evaluation.

## Multi-asset extension rule

v1.1 extends v1.0.4 by making trigger normalization asset-aware. Each eligible AssetProfile must carry or reference a versioned `AssetTriggerProfile` so the same Trigger Framework can be calibrated to that asset's normal distribution.

`Same Trigger Framework + Different Asset Calibration`.

Scanner discovery or Research Priority never substitutes for a Market Pulse trigger, Trade Qualification, Auto Trade Admission or Portfolio Exposure decision.
