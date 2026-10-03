# Implementation Contracts Delta v1.0.4

## MarketPulseSnapshot

Fields should include enough normalized data to evaluate the four P0 trigger families without running the full Alpha model:

- symbol
- ts
- order_flow_level / delta / normalized
- aggressor_imbalance / trade_intensity / normalized
- book_imbalance / depth_depletion / normalized
- return_velocity / acceleration / normalized
- spread / liquidity context
- data_freshness
- config_version

## RuntimeState

Enum:

- `IDLE`
- `CANDIDATE`
- `ACTIVE`
- `IN_POSITION`

Runtime state is operational and must not replace Prediction Episode state.

## TriggerDecision

- triggered: boolean
- trigger_families[]
- severity_by_family
- reason_codes[]
- evaluated_at
- normalization_config_version

## TriggerAnalyticsLink

Link trigger_id to:

- alpha_evaluation_id
- prediction_episode_id optional
- qualification decision optional
- admission decision optional
- trade_id optional
- prediction outcome optional
- trade outcome optional

## Invariant

A `TriggerDecision(triggered=true)` authorizes only a Full Alpha Evaluation. It does not authorize a TradePlan or order.
