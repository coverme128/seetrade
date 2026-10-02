# Personal Config Baseline

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
