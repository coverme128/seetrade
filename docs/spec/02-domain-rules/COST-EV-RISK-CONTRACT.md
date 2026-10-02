# Cost, EV & Risk Contract

## 1. Net EV

理论计算至少包含：

Gross Expected Move / Edge
- Spread Cost
- Entry Fee
- Exit Fee
- Expected Slippage
- Market Impact
- Adverse Selection
- Funding Cost（适用时）
= Net Expected Edge

## 2. Required Economics

每个 Trade Plan 必须有：

- Gross Expected Edge
- Total Expected Cost
- Net Expected Edge
- Expected Net P&L
- Expected Loss
- Planned Max Loss
- Break-even probability
- Stop Distance
- Expected MAE
- Liquidity capacity
- Liquidation distance/buffer

## 3. Risk Principles

- No Trade Without Positive Net EV
- No Leverage Without Risk Budget
- No Rebalance Without Recalculation
- No Add Because of Loss
- No Hold Because of Hope
- No Martingale
- No averaging down without fresh independent Alpha

## 4. Position / Leverage

杠杆不是 Alpha 输入，而是仓位相对于 Equity 的结果：

`Effective Leverage = Position Notional / Account Equity`

先算 Risk Budget → Stop/MAE → Position → Effective Leverage。

## 5. Rebalance

每秒可重算 Target Position，但必须有：

- Minimum position change threshold
- Minimum rebalance interval
- Maximum adjustments per trade
- Minimum incremental EV
- hysteresis / deadband

避免频繁交易吞噬 Edge。
