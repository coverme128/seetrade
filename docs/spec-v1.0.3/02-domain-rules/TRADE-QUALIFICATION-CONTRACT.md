# Trade Qualification Contract

## 1. Purpose

Trade Qualification Gate 是 Recommendation 进入 Trade Plan 的唯一正式门。

## 2. Required Gates

至少必须通过：

1. Probability Edge
2. Calibration
3. Minimum Net EV
4. Expected Net P&L
5. Cost Confidence
6. Liquidity
7. Slippage / Spread
8. Volatility
9. Risk Budget
10. Model Regime / OOD
11. Market Data Freshness
12. Exchange Health

## 3. Rule

`QUALIFIED` 只有在所有 hard gates 满足时成立。

不得用一个平均总分覆盖 hard fail。

## 4. Example

即使：

`P(up)=68%`

但：

`Net EV <= 0`

必须输出：

- Core View: BULLISH
- Trade Qualification: NOT QUALIFIED
- Reason: costs remove expected edge

## 5. In-Position Requalification

持仓后 Gate 持续重算，并可触发：

- ADD
- HOLD
- REDUCE
- EXIT

不允许“入场后只等 TP/SL”。


## 6. Relationship to AUTO

`QUALIFIED` 只证明机会本身通过交易资格，不代表必须交易。

- OBSERVE：继续跟踪，不执行。
- PAPER：可按 Paper 策略模拟。
- CONFIRM：向用户呈现 Trade Plan，由用户 Execute/Skip。
- AUTO：必须再经过 `Auto Trade Admission Gate`，随后仍需 Risk Gate。

Trade Qualification 不负责控制每日交易次数，也不负责在多个 Qualified 机会之间分配真钱执行权。
