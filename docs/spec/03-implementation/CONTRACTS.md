# Core Data Contracts

## MarketEvent

- exchange
- symbol
- event_type
- exchange_ts
- receive_ts
- sequence_id
- payload

## FeatureSnapshot

- feature_snapshot_id
- symbol
- ts
- feature_schema_version
- all first-phase features
- data_quality flags

## Prediction

- prediction_id
- symbol
- ts
- horizon
- target / opposite barrier
- raw probabilities
- calibrated probabilities
- expected edge before cost
- expected edge after cost
- model_version
- feature_snapshot_id
- status

## Qualification

- prediction_id
- gate version
- each gate status
- final status
- reason codes

## TradePlan

- plan_id
- prediction_id
- direction
- position_notional
- effective_leverage
- expected_net_pnl
- planned_max_loss
- entry rules
- TP/SL/timeout
- de-leverage rules
- exit rules

## Execution

- order_id
- exchange_order_id
- plan_id
- side
- qty
- price
- expected_price
- fill_price
- fee
- slippage
- send_ts
- ack_ts
- fill_ts

## Outcome

- prediction_id
- outcome_type
- first_hit_ts
- MFE
- MAE
- final_move
- prediction_error

## ModelVersion

- model_version
- model_type
- artifact_hash
- feature_schema_version
- label_version
- training_window
- validation_window
- calibration_version
- status: challenger/shadow/champion/retired
