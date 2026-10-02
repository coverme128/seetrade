# Personal Config Baseline v1.0.3

## Core

- exchanges: Binance, OKX
- symbols: BTCUSDT, ETHUSDT (normalized internal symbols allowed)
- AI_MODE: OFF by default
- trading_mode: OBSERVE by default

## Risk Envelope

Must be user-configurable but versioned/audited:

- max_account_capital_for_system
- max_risk_per_trade
- max_daily_loss
- max_drawdown
- max_position_notional
- max_effective_leverage
- min_liquidation_buffer
- max_slippage
- min_net_edge
- max_rebalances_per_trade

No default high leverage values should be shipped.


## Runtime Cadence

Frozen defaults for V1:

- alpha_eval_interval_ms: `1000`
- qualification_eval_interval_ms: `1000`
- ui_live_refresh_ms: `1000–2000` implementation range
- feature_update: event-driven / incremental

Episode creation/closure thresholds and hysteresis values remain research-configurable and versioned; do not invent universal probability thresholds in code.


## AUTO Admission

Required versioned config fields (no aggressive defaults):

- auto_min_net_edge
- auto_required_model_trust
- auto_max_concurrent_positions
- reentry_cooldown_seconds
- max_auto_entries_per_window
- max_daily_trades (safety cap only)
- admission_priority_policy_version

`auto_min_net_edge >= min_net_edge` must be enforced. Values are research/config outputs, not hard-coded universal constants.
