# Opportunity Radar — Product Spec

## Purpose

Opportunity Radar answers:

> Among assets already admitted to the active universe, where is the strongest currently qualified short-horizon setup?

It is downstream of Asset Qualification and upstream of the detailed Prediction Card.

## Radar Content

Each row/card shows only the information needed to decide what deserves attention:

- Symbol
- Direction: LONG / SHORT / FLAT
- Horizon
- Target / Stop
- calibrated P(up/down/timeout)
- Net Expected Edge
- Model Trust
- Trade Qualification status
- top 2–4 WHY NOW drivers
- mode availability: OBSERVE/PAPER/CONFIRM/AUTO

Example:

```text
SOLUSDT
QUALIFIED LONG
30s · +0.20% before -0.10%
P↑ 65% · P↓ 23% · timeout 12%
Net EV +0.031%
Model Trust GOOD
Drivers: Aggressive Buy ↑ · Microprice ↑ · Liquidity GOOD
```

## Status Ordering

Default sorting is not "largest predicted price move". Use:

1. QUALIFIED
2. EMERGING
3. WATCH
4. NO EDGE / DEGRADED

Within the same state, deterministic ranking may consider Net EV, model trust, execution quality and recency. Never rank by leverage.

## Drill-down

Selecting an asset opens the inherited v1.0.4 detailed card:

`Core Recommendation → WHY NOW → Trade Qualification → Trade Plan → Observe/Confirm/Auto`.

The radar itself never bypasses this detailed contract.


## Runtime Visibility

Radar is an **opportunity** surface, not an Eligible Universe inventory. An asset may be `ELIGIBLE` but remain invisible from the default live Radar while its runtime state is `IDLE` and no formal Episode exists.

The UI may show compact universe counters such as:

```text
Eligible Assets       18
Triggered/Candidate    3
Active Episodes        2
Qualified              1
```

Do not render every Market Pulse observation as a card. Triggered-but-no-edge events belong to diagnostics/history, not the default opportunity feed.

## AUTO Selection Display

When AUTO is enabled, the Radar may show a compact execution state per Qualified opportunity:

- `AUTO ADMITTED`
- `AUTO SKIPPED`

Do not hide Qualified-but-skipped opportunities. This preserves the distinction between opportunity quality and actual capital allocation.
