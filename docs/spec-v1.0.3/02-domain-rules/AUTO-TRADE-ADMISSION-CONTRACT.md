# Auto Trade Admission Contract v1.0.3

## 1. Purpose

`Trade Qualification` 与 `Auto Trade Admission` 必须分离。

- Trade Qualification 回答：**这笔机会本身是否值得交易？**
- Auto Trade Admission 回答：**在 AUTO 模式下，这笔已 Qualified 的机会是否值得现在占用真实资本和风险预算？**

`QUALIFIED != AUTO_EXECUTE`。

## 2. Inputs

Admission 只能读取已存在的确定性/量化输出：

- latest Qualification status
- calibrated probability
- expected net edge after cost
- expected net P&L / planned max loss
- Model Trust
- execution health
- spread / expected slippage / liquidity
- current account risk budget
- current positions/exposure
- episode age/freshness
- re-entry/churn state

AI/LLM 不得产生或覆盖 Admission 决策。

## 3. Hard Rules

AUTO 只有在以下条件全部满足时才可 `ADMITTED`：

1. latest `TradeQualification == QUALIFIED`；
2. Episode 未结束、未 invalidated、market data fresh；
3. Net EV 达到独立的 `auto_min_net_edge`；该门槛不得低于普通 Qualification 门槛；
4. Model Trust 达到 AUTO live 所需状态；
5. spread/slippage/liquidity/exchange health PASS；
6. Risk Budget 与 Position caps PASS；
7. 不违反 re-entry cooldown / max churn / max concurrent risk；
8. 下单前重新读取最新 Evaluation，机会仍满足以上规则。

任一 hard fail => `SKIPPED`。

## 4. Concurrent Candidates

如果多个机会同时 ADMISSION-eligible，而资金/风险预算不足以全部执行：

- 不要求全部交易；
- 使用透明、确定性、版本化的 priority tuple；
- V1 推荐优先比较：`net_edge_after_cost desc → model_trust desc → execution_quality desc → risk_consumption asc → episode_opened_at asc`；
- 不使用单一黑盒总分覆盖 hard fail；
- 不使用 LLM 排名真钱交易。

## 5. Daily Trade Count

`max_daily_trades` / `max_orders_per_window` 只作为风险与 churn 上限。

禁止：

- 为达到某个目标交易次数主动寻找低质量交易；
- 因“今天交易次数太少”降低 Qualification/Admission 门槛；
- 因连续亏损扩大 Admission 或仓位。

允许 0 trades/day。

## 6. Re-entry / Churn Protection

同资产刚刚退出后，重新进入必须同时满足：

- 新的/重新成立的独立 Edge；
- Fresh Qualification；
- Fresh Admission；
- cooldown 或 equivalent hysteresis 通过；
- 预期新增 Net EV 足以覆盖新增 round-trip cost。

## 7. Outputs

`AutoTradeAdmissionDecision`：

- episode_id
- evaluated_at
- decision: `ADMITTED | SKIPPED`
- reason_codes[]
- priority_tuple snapshot
- competing_episode_ids[] (optional)
- risk_budget_before
- risk_budget_after_if_admitted
- config_version

## 8. Audit Invariant

任何 AUTO live order 必须可追溯：

`PredictionEpisode → TradeQualification → AutoTradeAdmission(ADMITTED) → RiskDecision(PASS) → TradePlan → Execution`。

如果 Admission 缺失或为 SKIPPED，不允许发出新的 AUTO live entry order。
