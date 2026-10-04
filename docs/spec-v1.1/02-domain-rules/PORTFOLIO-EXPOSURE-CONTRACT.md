# Portfolio Exposure Gate Contract

## Purpose

An individually qualified trade may still be unsafe when combined with existing positions. Portfolio Exposure Gate is a mandatory downstream gate before live execution.

## Inputs

- account equity
- current positions and notionals
- current/target effective leverage
- direction by asset
- planned stop and per-trade risk
- per-asset volatility/tail metrics
- recent cross-asset correlation/common-move proxy
- exchange concentration
- pending orders

## Minimum hard controls

- `max_concurrent_positions`
- `max_total_gross_notional`
- `max_total_effective_leverage`
- `max_total_risk_budget_used`
- `max_same_direction_exposure`
- `max_per_asset_notional`
- `max_per_asset_risk`
- `max_exchange_concentration`

## Common-market concentration guard

V1.1 does not need a complex factor model. A simple deterministic guard is sufficient:

- maintain rolling return correlation/common-direction metrics among active assets;
- if multiple proposed positions are highly correlated and same-direction, reduce or block incremental exposure;
- record the reason.

## Output

- PASS / REDUCE / BLOCK
- approved_target_notional
- approved_effective_leverage
- reason_codes[]

The Portfolio Gate may reduce a TradePlan but may never increase its size beyond the individual Risk/Position Engine recommendation.


## Relationship to v1.0.4 Auto Trade Admission

For AUTO new entries, Portfolio Exposure Gate is evaluated after an `ADMITTED` decision and remains a mandatory downstream veto. Admission decides whether the candidate deserves current capital; Portfolio Exposure checks whether the resulting combined book is still within portfolio constraints.

For CONFIRM/manual execution, Portfolio Exposure Gate still applies even though Auto Admission is not used.
