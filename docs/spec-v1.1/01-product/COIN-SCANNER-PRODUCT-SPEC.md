# Coin Opportunity Scanner — Product Spec

## Purpose

The Scanner answers one product question:

> Which coins are currently worth admitting into Micro Alpha research and continuous monitoring?

It does not answer "what should I buy now?". That remains the job of the Core Recommendation + Trade Qualification pipeline.

## User Jobs

The single user can:

- see which markets were scanned;
- understand why a coin is eligible, collecting, rejected, degraded or suspended;
- manually add a coin to evaluation;
- trigger a rescan;
- inspect asset-quality metrics;
- start/stop research collection for a candidate;
- see which assets are actually active in the Alpha engine.

## Scanner Page

Recommended information hierarchy:

```text
COIN SCANNER
Last Scan     09:15:00
Next Scan     09:20:00
Scanned       186
Eligible       12
Collecting      5
Rejected      169

[Scan Now]  [+ Add Coin]
```

Table columns:

- Symbol
- Exchange availability
- Asset lifecycle state
- Liquidity quality
- Median spread
- Depth quality
- Estimated slippage
- Volatility/tail-risk state
- Data readiness
- Research priority
- Primary reason / blocker
- Action: View

Do not put leverage, BUY/SELL or live trade buttons in this table.

## Research Priority

Research Priority is allowed as a deterministic ranking for "what should the system collect/evaluate next".

Values:

- HIGH
- MEDIUM
- LOW
- REJECT

It MUST NOT be presented as trade confidence or investment attractiveness.

## Manual Add

UI:

```text
Add Coin
Symbol       SUIUSDT
Markets      Binance ✓  OKX ✓

[Start Evaluation]
```

After add:

`MANUAL → DISCOVERED → COLLECTING → CALIBRATING → OBSERVE ...`

If exchange validation fails, show the exact reason. If only one configured venue is available, the asset may remain researchable if the Asset Qualification contract permits it; missing cross-venue features must be explicitly marked unavailable rather than fabricated.

## Stable UX

Scanner results can change frequently, but universe membership should not visibly thrash.

UI should distinguish:

- Latest scanner observation
- Current lifecycle state
- Pending transition
- Transition reason

Example:

```text
SOLUSDT
Latest scan: HIGH priority
Lifecycle: OBSERVE
Pending: ELIGIBLE candidate
Reason: awaiting minimum calibration evidence
```
