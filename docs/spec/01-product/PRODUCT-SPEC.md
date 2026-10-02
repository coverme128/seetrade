# Product Spec

## 1. User

单一个人用户，目标是使用真实市场数据发现短周期可交易机会，并可从观察逐步过渡到自动交易。

## 2. Core Jobs

系统首先回答：

1. 现在是否存在值得关注的短周期方向性机会？
2. 未来多久、先上涨/下跌多少的条件概率是多少？
3. 是什么市场驱动因素推动这个判断？
4. 扣除手续费、滑点、Spread、Adverse Selection 后还有没有正 Net EV？
5. 当前是否已经达到 Trade Qualification？
6. 若可交易，建议仓位、有效杠杆、TP/SL/Timeout、减仓与退出条件是什么？
7. 用户要 OBSERVE、PAPER、CONFIRM 还是 AUTO？
8. 最后预测与真实结果、预期与真实 P&L 是否一致？

## 3. Main Screen

主界面只保留 5 个核心区块：

### A. LIVE MARKET
- Symbol
- Exchange health
- Mid / Spread
- Volatility regime
- Liquidity state

### B. CORE RECOMMENDATION
必须以用户可理解的语言表达：

- Direction: LONG / SHORT / FLAT
- Horizon
- Target
- Opposite barrier / Stop reference
- P(up first)
- P(down first)
- P(timeout)
- Calibrated probability

示例：

> 未来 120 秒，BTC 先上涨 +0.50% 的校准概率为 63%，先下跌 -0.20% 的概率为 22%，超时未触发为 15%。

### C. WHY NOW
展示最关键 Drivers，不展示几十个指标：

- Order Book Imbalance
- Order Flow Imbalance
- Aggressive Trade Imbalance
- Microprice vs Mid
- Short-term Momentum
- Spread
- Depth/Liquidity
- Realized Volatility
- Cross-Venue Lead/Lag
- Liquidation Flow（可用时）

默认以 Strong/Positive/Negative/Normal/Weak 等解释性状态展示；可展开 Raw Metrics。

### D. TRADE QUALIFICATION
显示核心 Gate：

- Probability Edge
- Calibration
- Net EV
- Expected Net P&L
- Cost
- Liquidity
- Slippage
- Volatility
- Risk Budget
- Model Regime / OOD

最终状态：

- WATCH
- EMERGING
- QUALIFIED
- NOT QUALIFIED

### E. NEXT ACTION
只有 QUALIFIED 才生成完整交易建议：

- Suggested Position
- Effective Leverage
- Expected Net P&L
- Planned Max Loss
- Entry type
- TP
- SL
- Max Hold
- De-leverage conditions
- Exit conditions

并显示：OBSERVE / PAPER / CONFIRM / AUTO。

## 4. Modes

### OBSERVE
不下单。持续跟踪 Prediction 到 Outcome。用于验证模型和人工看盘。

### PAPER
系统自动执行模拟交易，计算手续费、滑点、成交质量与 P&L。

### CONFIRM
系统生成完整 Trade Plan，用户只决定 Execute / Skip；不允许临时随意把系统建议的 1.4x 改成 20x。

### AUTO
系统在用户预先设定的 Risk Envelope 内自动 Entry / Add / Hold / Reduce / Exit。

## 5. Prediction Lifecycle

状态机：

`NO_SIGNAL → WATCH → EMERGING → QUALIFIED → TRADE_READY → TRACKING → COMPLETED`

持仓后允许：

`IN_POSITION → HOLD / ADD / REDUCE / EXIT`

每秒重算，但 UI 只在显著变化时更新核心状态。

## 6. Stable Display Principle

- 后台 Alpha/Gate 约 1s 重算
- UI Live Metrics 可 1–2s 更新
- Core Recommendation 只在跨阈值/状态变化时变
- 使用 hysteresis / debounce，避免 Card 闪烁与频繁反向
- 正式 Prediction 不是每秒创建，而是状态事件触发

## 7. Result Card

每个 Prediction 结束后原卡转为 Result：

- Expected
- Actual
- TARGET_HIT / STOP_HIT / TIMEOUT / STRONG_OPPOSITE_MOVE
- MFE
- MAE
- Prediction Error
- Predicted Probability vs Actual
- Whether added to Learning Dataset

## 8. Track Record

主界面提供：

- Last N comparable predictions
- Predicted hit rate
- Actual hit rate
- Calibration
- Expected Net Edge
- Realized Net Edge
- Paper/Live Net P&L
- Model Drift

## 9. AI UX

AI_MODE=OFF：完整产品可运行。

AI_MODE=ASSIST：仅增加：
- Why Now 解释
- Trade Review
- Loss Review
- Daily Summary
- Model Drift / anomaly summary

不得改变核心量化结果。
