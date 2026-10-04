# Asset Trigger Profile Contract

## Purpose

v1.1 reuses the v1.0.4 Trigger Framework across a dynamic Coin Universe without applying BTC/ETH absolute thresholds to every asset.

The contract is:

`Same Trigger Framework + Different Asset Calibration`.

## Required fields

```text
asset_trigger_profile_id
asset_id
profile_version
normalization_window
metric_distribution_version
order_flow_thresholds
aggressive_trade_thresholds
book_depth_thresholds
price_velocity_thresholds
risk_trigger_thresholds
data_health_thresholds
min_candidate_hold_ms
trigger_debounce_ms
created_at
updated_at
```

Exact storage shape may be JSON/config objects in the existing single-user SQLite architecture. Do not create a new service solely for this profile.

## Severity semantics

Each Alpha Trigger family may resolve to:

`NORMAL / ABNORMAL / EXTREME`.

The inherited first production combination rule remains:

`ANY EXTREME OR at least 2 distinct ABNORMAL trigger families => Full Alpha Evaluation`.

The thresholds are research/calibration outputs. Do not hard-code one absolute price/volume threshold across all coins.

## Data requirements

A new Coin may be discovered before it has enough history to calibrate a stable Trigger Profile. During `COLLECTING/CALIBRATING`, it may collect Market Pulse statistics and run OBSERVE research, but it MUST NOT become AUTO-live merely because a generic default threshold fired.

## Boundaries

- Trigger Profile does not contain buy/sell logic.
- Trigger Profile does not alter Model Probability.
- Trigger Profile does not bypass Asset Qualification or Trade Qualification.
- Trigger Profile does not rank coins by expected profit.
- AI/LLM may explain a trigger after the fact but does not set production trigger severity.

## Auditability

Every trigger event must reference the exact AssetTriggerProfile version and normalized metric snapshot used to produce the severity.
