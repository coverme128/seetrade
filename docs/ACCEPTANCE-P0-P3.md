# v1.0.3 文档 P0–P3 完整实现核对 · 2026-10-02

本次采用 [原始实施计划](spec-v1.0.3/03-implementation/IMPLEMENTATION-PLAN.md) 的 P0–P3 定义，共 12 个阶段。个人本地架构保留 Rust / Python / Next.js / SQLite / 本地 Parquet。PostgreSQL、对象存储和第三方训练库属于推荐技术选型，未因本次交付引入额外基础设施。

下表的 PASS 表示代码与本地功能验证通过，包含合成数据、确定性回放和本机模拟交易所；不等于实盘门禁通过。真实样本、私有模拟账户与小额实盘证据另列，不能从代码测试推断盈利能力。

## 阶段实现与证据

| 文档阶段 | 实现范围与入口 | 功能验收 |
|---|---|---|
| P0 / 1 Market Data + Replay | `feeds.rs`、`book.rs`：Binance / OKX，BTC / ETH，BBO / L2、主动成交、mark / index；序列校验、断档恢复；`storage.rs` 保存规范事件；`research/parquet.py --watch` 持续生成压缩 Parquet；JSONL 保留完整命令和 checkpoint 以恢复交易决策 | **PASS**：序列桥接、断档、时间戳与来源隔离；Parquet 往返；固定录制直接计算 / 重放摘要相同 |
| P0 / 2 Feature + Triple Barrier | `core.rs`、`cross_venue.rs`、`horizon.rs`：九类首批可用特征，跨场所背景，First-hit、MFE / MAE、自动 Outcome 与到期观察 | **PASS**：首触顺序、迟到行情不得重写标签、数据失效排除、独立到期观察、领先滞后双向 / 同步 / 缺数据测试 |
| P0 / 3 Prediction + OBSERVE | 确定性评分、研究 Logistic、冻结概率校准；每秒评估与 Episode 发布分离；滞回确认；WHY NOW、当前建议、结束结果 | **PASS**：评估不增加独立样本、开场快照不被覆盖、OBSERVE 无订单也闭合结果、重启不重复结果、桌面 / 手机界面测试 |
| P0 / 4 Trade Qualification | `core.rs`：概率、校准、模型信任、成本后 EV / 预期净收益、成本、深度、价差 / 滑点、波动、风险、OOD、新鲜度、交易所健康逐项门槛 | **PASS**：高概率负 EV、未校准、LOW、过期、预算不足均拒绝；hard fail 不能被总分覆盖 |
| P1 / 5 Model Health | `model.rs`：校准桶、Brier、近期 / 基线、市场状态分组与 OOD、预测 EV、PAPER 执行成本 / 实际 EV、实盘成本 / 实际 EV；降信任和重训建议 | **PASS**：LOW 禁止新风险、DEGRADED 减仓；反馈不修改权重；PAPER 与实盘独立统计，按配置 / 模型筛选可比证据 |
| P1 / 6 Cost + PAPER | `core.rs`：手续费、价差、滑点、冲击、逆向选择、资金费；按盘口模拟成交、部分成交；执行记录及实际净收益 | **PASS**：费用 / 资金费一次结算、薄簿分批退出、过期行情不成交、预期 / 实际成本和收益可还原 |
| P1 / 7 Position / Leverage / Exit | `risk.rs` 提供明确的模拟 / 真实账户上下文，共用资格与仓位公式；当前 / 目标名义金额和杠杆，ADD / HOLD / REDUCE / EXIT；间隔、死区、次数、独立信号和原止损预算 | **PASS**：亏损不能触发加仓；信任降低后即使受小额上限限制也降低目标；实盘日损益和可用资金不与模拟账本混用；其他挂单预留资金；反向建议退出 |
| P1 / 8 AUTO Admission | `admission.rs`：独立 ADMITTED / SKIPPED，版本化净优势阈值、确定性竞争排序、风险预留、冷却 / 新 Alpha、每日与窗口上限 | **PASS**：QUALIFIED 可被跳过但保留客观 Outcome；下单前复核；可追溯原因和竞争快照；上限不驱动凑单 |
| P2 / 9 CONFIRM Small Live | `broker.rs`、`live.rs`、`main.rs`：Execute / Skip、独立小额上限、签名 REST、精度与合约单位、部分成交 / 拒单 / UNKNOWN、账户 / 费用 / 资金费、延迟、撤单、Kill | **PASS（本机模拟）**：两家适配器账户 → 下单 → 查询成交 → 撤单 → reduce-only → 拒单请求链；CONFIRM 发送前重算资格和真实账户预算；每单持久化后发送 |
| P2 / 10 AUTO | 显式环境权限与模式确认，共用 Risk Gate；准入后入场、独立信号加仓、减仓和退出；异常熔断、状态恢复 | **PASS（本机模拟）**：LOW / 过期 / 数据异常 / 风险或准入失败不下单；慢账户请求不挡保护退出；UNKNOWN 不重发；kill 取消新增风险；部分成交后持续退出 |
| P3 / 11 Controlled Adaptation | `research.rs`、`research/train.py`、`shadow.rs`：定时 / 手动单任务、score → Logistic → GBT、walk-forward / OOS、分组校准、影子配对、比较报告、受控晋升 / 回退 | **PASS**：未来数据不影响拟合；训练 / 校准 / 测试隔离，测试窗口连同标签路径不重叠；失效结果不转为 timeout；工件 hash 和报告配对；不合格证据不晋升；定时任务不自动换 Champion |
| P3 / 12 Optional AI Assist | `assist.rs`、`/api/assist`：显式 OFF / ASSIST；本地模型只读取 Why Now、交易 / 亏损、每日摘要、漂移及研究证据；无模型时提供确定性摘要 | **PASS**：OFF 在已配置模型名时仍禁用模型调用；ASSIST 需模型配置；解释接口不能改变概率、EV、仓位、准入或订单 |

OI、强平流属于可靠性有条件的输入，依用户范围不扩展。跨场所领先滞后仅作为可审查背景，未增加模型维度。没有新增 Transformer / LSTM / RL、在线改权重、自动晋升或 UI 重做。交易所原生保护单不是文档明确指定的实现方式，本轮不计入强制缺口。

## 本轮修正原因与影响

1. 真实账户权益原已接入，但资格中的日损益、回撤基准、其他仓位风险和可用资金预留未完全与 PAPER 隔离。本轮通过 `RiskContext` 明确输入来源，保留同一仓位计算公式；PAPER 账户显示 / 峰值恢复为独立账本。
2. DEGRADED 原先仅缩小风险预算，在名义金额上限先起作用时可能不缩小目标。现在对受全部上限约束的目标应用仓位乘数。
3. PAPER 反馈原有实际成本偏差，缺少独立实际净收益偏差；加仓后预期优势未按新增资金加权。现单独统计最近 50 个可比交易，达到样本门槛后参与降信任；保留 Prediction EV 的独立语义。
4. CONFIRM / ADD 原先发送时依赖外层状态检查；现增加核心内的新鲜资格 / 目标仓位复核。相反方向的有效建议会触发原持仓退出。
5. 研究报告导入原检查测试起点递增但未验证前一个终点；现检查真实边界，并在 Python 端剔除跨越下一测试窗口的标签。无效 / 未知标签及不兼容特征 schema 不可进入训练。
6. AI 原通过模型名隐式启用；现提供文档规定的 OFF / ASSIST，兼容旧的显式模型名配置。

影响合同：Trade Qualification、Cost/EV/Risk、Learning、Feature/Model、AUTO Admission、Core Data Contracts、UI Card、Live Gates。原始规范文件保持原样。

## 验证命令

```bash
source scripts/env.sh
./scripts/check.sh
# 核心与前端已运行后；仅在没有私有执行账户的测试实例运行
(cd web && npm run test:e2e)
```

`check.sh` 检查 Rustfmt、Rust 测试、Clippy、前端格式 / TypeScript / 生产构建、直接计算 / 离线回放 / 固定摘要三方一致，以及 Python 研究测试。浏览器测试会临时调整测试配置并恢复；不要对正在使用真实资金的实例运行。

最新实际结果和运行快照见 [p0-p3-smoke.json](p0-p3-smoke.json)。本轮为 78 项 Rust、12 项 Python、14 项浏览器测试；固定 240 秒录制 3,122 条输入、7 个闭合结果、0 笔交易。合成控制流验证不构成策略收益证据。

## 生产门禁与仍缺的证据

- OBSERVE、模型健康、PAPER 的功能门禁测试通过；真实市场中的校准 / 样本充分性依实际数据决定。未校准模型仍会保持 LOW / NOT QUALIFIED，不为了演示交易降低门槛。
- **CONFIRM_LIVE_READY：FAIL / 未完成环境验收**。没有配置私有账户、没有交易所模拟账户联调和小额实盘记录；本机模拟适配测试不能替代这些证据。
- **AUTO_LIVE_READY：FAIL / 未启用**。尚无满足配置样本量和多窗口正收益要求的实际验证证据；默认 OBSERVE，显式权限和运行门槛继续生效。
- **RETRAIN_PROMOTION_READY：FAIL / 无已通过证据的真实候选**。生产 Champion 未改变。候选仍须离线和影子报告通过、当前行情 / 账户条件满足并由用户明确晋升。
- 本地 TP / SL / 超时保护依赖进程运行；本机停止或交易所不可达不能保证退出。未将此限制描述成已完成交易所端止损。

## 迁移与回退

没有数据库 DDL 迁移。旧 checkpoint 缺少的 `paper_samples` / `paper_ev_drift` 使用默认值；本轮后新产生的加仓按金额冻结预期优势，历史结果保持原记录，不伪造补算。重启保持 OBSERVE 并使中断行情 / 预测失效；未确认提交继续以 UNKNOWN 对账。

增加 `MICRO_ALPHA_AI_MODE=OFF` 即可明确关闭模型调用；启用 ASSIST 需指定已有本地模型。旧版只配置 `MICRO_ALPHA_ASSIST_MODEL` 的显式启用方式仍兼容；不会自动安装或下载模型。

旧研究报告若测试路径重叠，将被拒绝，应重新运行研究而非手改 PASS 标志。新摘要因健康字段和风险语义变化重新冻结，直接计算与重放必须相同。

回退代码前先停止实例并保存数据目录副本；仅在 OBSERVE 且无未完成真实订单 / 持仓时恢复对应版本的数据副本。已有真实账户状态必须以交易所重新对账为准，不能用旧 checkpoint 回滚实际成交。模型回退使用现有受控 `model_rollback`，不回滚损益。
