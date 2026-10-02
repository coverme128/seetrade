# 个人执行账户配置

公共行情、OBSERVE、本地 PAPER 不需要密钥。私有执行适配器支持 Binance USDⓈ-M 和 OKX USDT SWAP，一个数据目录对应一个独立账户；同时只选一家执行交易所。交易信号仍同时观察两家公共行情。

复制 `.env.example` 为 `.env.local`，在本机填写；文件已被 Git 忽略。Binance 使用 Key / Secret；OKX 另需 API Passphrase。程序不读取网页登录密码，密钥不进入状态 API、数据库、录制或网页。默认不配置执行账户。

```bash
set -a
source .env.local
set +a
./scripts/dev.sh
```

先使用 `MICRO_ALPHA_EXECUTION=binance` 或 `okx`，保留 `MICRO_ALPHA_EXECUTION_ENV=testnet`。Binance 使用 `https://demo-fapi.binance.com`；OKX 使用模拟交易请求头，必须匹配模拟账户密钥。这与本地 PAPER 是不同的账户。公共信号来自真实公共行情，交易所模拟环境的价格和成交条件可能不同。

支持单向持仓、USDT 保证金、全仓、交易所杠杆不超过 3。OKX 要求 Futures mode（账户级别 2），不接受组合保证金。账户应专用于本程序。代码读取并核对模式，不主动改变交易所账户设置。名义金额另受 `MICRO_ALPHA_LIVE_CAP` 限制，默认 100 USDT，允许 10–500 USDT；最小数量和名义金额不够时拒单，不会向上取整突破风险上限。

填写凭证不等于开放下单：默认仍是 OBSERVE。新预测需通过校准、可信度、成本后 EV、两家行情和风险门槛，账户对账也必须通过，才能在 CONFIRM 中点击“执行当前计划”。预测和意图有有效期，发送前再次检查。真实资金端点另要求 `MICRO_ALPHA_EXECUTION_ENV=live` 和 `MICRO_ALPHA_ALLOW_LIVE=YES`。AUTO 另要求 `MICRO_ALPHA_ALLOW_AUTO=YES`、界面显式确认，以及至少 `max(high_trust_samples,90)` 笔同配置 PAPER 交易、三个连续窗口净收益为正、尾损 / 执行成本 / 当前模型健康检查通过。

订单处理采用 REST：意图落盘 → SUBMITTING 落盘 → 提交 IOC 限价 → 查询订单和成交 → 账户对账。ACK 不是成交；部分成交保留实际数量，超时进入 UNKNOWN 并按原 client ID 查询，不重发。明确拒绝与未知结果分开。交易费用、资金费和完成交易结果记录在 `broker_order`、`broker_snapshot`、`live_trade` 中。

风险退出使用 reduce-only 市价单；ADD 要求新形成的独立信号、增量 EV、间隔、次数、原止损风险预算，亏损不能触发加仓。CONFIRM 的入场逐笔确认，持仓后的减仓和保护退出自动执行。AUTO 允许按同一门槛自动入场和独立 ADD。全局 kill 阻止新增订单并撤销本程序未完成的入场；hold 保留原 TP/SL/期限保护，flatten 排队减仓平仓。不会取消其他程序的订单。

账户轮询每次完成后等待 5 秒，账户信息超过 10 秒阻断新增风险并触发已管理持仓的保护退出。账户查询、交易规则刷新、每个标的的开仓 / 加仓通道、减仓通道和撤单任务相互独立；订单意图每 100ms 检查，未完成订单约每秒查询，网络耗时另计。保护订单不等待账户查询或新的规则 HTTP 请求；首次缺少规则仍拒绝，缓存规则过期则只允许减仓。止损、止盈和期限保护由本机进程监控；当前未实现交易所原生条件止损或私有 WebSocket。电脑休眠、进程终止或交易所不可达时，无法保证及时退出。重启后模式回到 OBSERVE，先对账，未完成提交按 UNKNOWN 恢复。订单或账户不一致不会自动“修复”为已成交。

目前私有流程的验收证据来自本地模拟交易所和领域测试；没有使用真实 API Key，尚未完成交易所模拟账户或小额实盘验收。代码支持这些流程，不代表已获得真钱运行的有效性证据。交易所手续费和微观流动性应与本地成本配置核对，校准 / PAPER / Shadow 未通过时不会因为配置密钥而绕过限制。

接口依据：[Binance 订单与撤单](https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/New-Order)、[Binance 测试环境](https://developers.binance.com/docs/derivatives/usds-margined-futures/general-info)、[OKX 订单状态与对账](https://www.okx.com/docs-v5/trick_en/)。


## v1.0.3 AUTO 准入

普通交易资格与 AUTO 资本分配独立。当前 Qualified Episode 还必须通过 `auto_min_net_edge`、可信度、最新 Evaluation、执行质量、预算、并发数、冷却与安全次数上限。默认 AUTO 净优势 0.0004、要求 HIGH、最大并发 1、冷却 60 秒、300 秒最多 3 次入场、每日最多 20 次；可在风险设置中版本化修改，AUTO 优势门槛不得低于普通资格门槛。

同资产退出后须出现新的信号 epoch，增量净优势还须覆盖新增完整往返成本与 incremental_ev。每日 / 窗口统计保守计入本程序创建的所有实盘 ENTRY 意图（含 CONFIRM、后续取消或拒绝）；限制只阻止新入场，保护性退出不受次数上限阻止。它们绝不是交易次数目标。

竞争排序 `priority-v1`：成本后净优势降序 → 模型可信度降序 → 预期执行质量降序 → 风险占用升序 → Episode 开场时间升序 → Episode ID 字典序。执行质量定义为价差、预期滑点和市场冲击之和的负值。Hard fail 不参与预算分配。预算包含持仓及未完成订单预留，可用资金不足时后续候选被 SKIPPED；准入后仍受实际订单精度、最小金额、可用资金及共享风控约束。

每笔 AUTO ENTRY 意图含 `execution_mode=AUTO` 和独立 `admission`。发送前在核心中重算最新 Evaluation 和准入，缺记录、过期、换 Episode、模式变更、门槛失败或预算不足均拒绝。`auto_admission`、`live_risk_decision`、`live_risk_recheck` 与 broker 回报可串联审计；`/api/history?kind=auto_admission&limit=50` 或界面“加载准入审计”可查询。当前 Card 的 SKIPPED 表示当前不再分配新风险，已创建订单的历史 ADMITTED 仍保留在订单中。

OBSERVE / PAPER 下的 SHADOW 准入只是使用模拟账本的确定性预演，不发送订单，也不替代真实账户权限、多窗口策略验证和交易所模拟 / 小额实盘联调。


## 保护执行与实盘反馈补齐

平仓量以已确认成交账本为准，不以过期账户快照推断成交；因此部分成交的 ENTRY / ADD 尚待核实时，已确认部分也能发出 reduce-only 退出。保护原因锁定到本程序成交仓位归零且没有待处理订单；同时撤销仍可能新增风险的本程序订单，后续确认的新成交继续退出。同一标的只允许一个待处理减仓；明确拒绝后至少间隔 5 秒重试，UNKNOWN 只查询，绝不盲目重发。小于交易所最小数量的尾仓会保留待处理原因，不能假装平仓成功。发送前的 SUBMITTING 必须完成持久化，写盘后再核对 kill / 模式 / 保护状态。

每笔新意图冻结模型版本、配置版本、当时中间价、预计完整往返成本和净优势。已闭合真实交易聚合 ENTRY、独立 ADD、所有分批减仓的实际成交、手续费和资金费。实际执行成本 = 各腿相对意图参考中间价的不利成交差 + 手续费 − 资金费收入；预期成本只对 ENTRY / ADD 的实际成交量计入各自完整往返预算，净收益预期也按每次加仓当时的信号加权。它包含从意图到成交的价格变化，不声称是纯交易所滑点。

模型健康中的 `live` 单独报告最近 50 个同模型 / 同配置闭合交易的名义金额加权执行偏差与 EV 偏差。成本超出 `max_slippage` 即降为 DEGRADED；EV 需至少 `min_samples` 再与 `ev_drift_limit` 比较。结果只能降低现有可信度，不能抵消 LOW，也不能被良好的 PAPER 成绩抵消。缺少冻结参考的旧记录和版本不匹配的记录显示为排除样本，不补造历史。资金费后续对账会更新同一交易及指标，不生成第二笔交易。

执行面板补充成交账本量 / 账户量、快照年龄、当前 / 已批准目标名义金额及有效杠杆、止盈止损、动作、保护原因和待处理状态。目标表示已批准订单执行后的持仓（保护退出目标为零），不是尚未经过真实账户风控的模型建议；待核实不等于已成交。


资格阶段现直接读取所用账户的预算上下文；PAPER 日损益 / 回撤不会污染真实资格，真实挂单也会预留可用资金。同标的排队订单在重新核对自身总目标时不重复扣减。DEGRADED 会缩小最终目标名义金额，即便原目标受小额上限约束。

CONFIRM 与 ADD 的 SUBMITTING 边界也重新计算最新资格、方向、真实账户预算与目标仓位；新增风险不得超过当前目标。有效建议反向时，已有真实持仓进入保护性退出。AUTO ENTRY 保留独立准入及其发送前复核。
