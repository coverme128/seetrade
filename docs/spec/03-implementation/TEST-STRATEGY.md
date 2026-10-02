# Test Strategy

## Unit

- order book apply delta
- sequence gap detection
- feature math
- label first-hit ordering
- EV calculation
- position sizing
- leverage caps
- gate logic
- no-martingale rule

## Replay Golden Tests

固定一组 market recordings，验证：

- same FeatureSnapshot
- same Prediction
- same Qualification
- same TradePlan

## Simulation

- spread widening
- WS gap
- stale data
- exchange disconnect
- slippage spike
- volatility spike
- partial fill
- API reject
- liquidation cascade

## Live Safety

- sandbox/testnet where applicable
- tiny notional live test
- max order size hardcoded env cap for first live phase
- kill switch drill
