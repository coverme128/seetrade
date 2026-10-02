# Product Spec v1.0.3

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
系统在用户预先设定的 Risk Envelope 内自动运行，但 **QUALIFIED 不代表自动执行**。

AUTO 流程：

`QUALIFIED → Auto Trade Admission → Risk Gate → EXECUTE / SKIP`

Auto Trade Admission 只从已经 Qualified 的机会中决定当前是否值得占用真实资本和风险预算。它不得修改原始 Direction / Probability / Net EV，也不得把 NOT QUALIFIED 机会提升为可交易。

## 5. Prediction Episode Lifecycle

后台约每 1 秒产生一次新的 Evaluation，但正式用户对象是 **Prediction Episode**。

状态机：

`NO_SIGNAL → WATCH → EMERGING → QUALIFIED → TRADE_READY → TRACKING → COMPLETED`

持仓后允许：

`IN_POSITION → HOLD / ADD / REDUCE / EXIT`

规则：

- 同一个连续机会只维护一个 Episode；不得每秒创建一条新的正式 Prediction。
- Evaluation 只更新 Episode 的概率、Net EV、Drivers、Qualification 与 live metrics。
- 只有状态跨阈值、方向/目标发生实质变化、旧 Episode 已结束或信号失效后重新形成，才允许创建新的 Episode。
- 每个 Episode 必须自动形成 Ground Truth Outcome，即使没有真实交易。
- Episode 结束后转为 Result Card，不在 Active 列表继续占位。

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

## 9. AUTO Selection Principle

AUTO 模式必须把“建议质量”和“是否现在执行”分开：

- `Trade Qualification`：这笔机会本身是否值得交易？
- `Auto Trade Admission`：即使值得交易，现在是否值得系统自动使用真钱执行？

最小准入条件：

- 当前 Episode 仍为 QUALIFIED
- 最新 Net EV 达到 AUTO 专用的更严格门槛
- Model Trust 满足 live-auto 要求
- execution health / spread / slippage / liquidity 均 PASS
- Risk Budget 可用
- 无 re-entry cooldown / churn violation
- 没有更高优先级机会正在占用冲突的风险预算

多个候选同时存在时，使用透明的确定性排序，不使用不透明 AI 总分。优先比较成本后 Net EV、Model Trust、执行质量和风险占用；具体 tie-breaker 必须版本化。

`max_daily_trades` 可以存在，但只能作为安全上限，不得为了达到次数主动交易。

## 10. Output Volume Principle

系统必须遵守：

`High-frequency Evaluation → Low-frequency Episode Publication → Fewer Qualified Opportunities → Selective Trades`

禁止把后台 evaluation count 直接展示为机会数量。用户首页只显示当前 Active/Qualified Episodes，历史结果进入 Review/History。

## 11. AI UX

AI_MODE=OFF：完整产品可运行。

AI_MODE=ASSIST：仅增加：
- Why Now 解释
- Trade Review
- Loss Review
- Daily Summary
- Model Drift / anomaly summary

不得改变核心量化结果。
