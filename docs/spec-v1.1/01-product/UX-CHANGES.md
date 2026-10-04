# v1.1 UX Delta

## Navigation

Recommended personal navigation:

- Dashboard
- Opportunities
- Coin Scanner **NEW**
- Predictions
- Trades
- Models / Model Health
- Settings

No SaaS workspace/admin navigation.

## Dashboard additions

Add:

1. Active Universe summary
2. Top current opportunities
3. Asset health warnings
4. Portfolio exposure summary

Do not turn the dashboard into an exchange-style ticker wall.

## Asset Detail

Add an Asset Profile section:

- current lifecycle state
- exchange availability
- data readiness
- model version
- calibration status
- Model Trust
- current cost/liquidity profile
- eligible execution modes
- transition history

## Portfolio Exposure Panel

When multiple trade-ready assets exist, show:

```text
Gross Exposure       1.8x equity
Long Exposure        1.5x
Short Exposure       0.3x
Open Positions       3 / max configured
Risk Budget Used     42%
Concentration        NORMAL
```

If Portfolio Exposure Gate blocks a trade, show the reason explicitly, e.g.:

`Trade qualified individually, but blocked: aggregate same-direction crypto exposure exceeded.`

## UX Priority

1. Explain asset eligibility.
2. Explain current alpha recommendation.
3. Explain why a trade is or is not qualified.
4. Explain portfolio-level blocking.
5. Only then show execution controls.
