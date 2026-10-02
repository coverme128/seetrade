# 交付与验收 · v0.3 / 合同 v1.0.3 · 2026-10-02

完整 P0–P3 的 12 阶段、修正内容、迁移 / 回退与实际门禁状态见 [ACCEPTANCE-P0-P3.md](ACCEPTANCE-P0-P3.md)。

升级包原文完整保存在 [spec-v1.0.3](spec-v1.0.3/MASTER-CONTRACT.md)，原 v1.0.1 文件未修改。本次覆盖升级包携带的 v1.0.2 Episode 要求及 v1.0.3 AUTO Admission 要求；P0 → P1 Admission → P2 执行 → P3 研究的优先级依照新版合同。原有实现与边界见 [v0.2 验收](ACCEPTANCE-v0.2.md)。

## v1.0.2 逐项对应

| 要求 | 实现与验证 |
|---|---|
| Alpha / Qualification 默认约 1 秒，特征事件驱动 | 两个独立 cadence 配置；生产 Clock 100ms 负责到期 / 保护检查，盘口事件即时维护特征；100ms Clock 测试不增加正式预测 |
| 连续机会只有一个 Episode，避免按秒复制 | signal epoch、滞回与确认时间；一个 epoch 发布一次；波动更新同一 ID；结果保留期不是重复发布计时器 |
| 开场与最新快照、屏障、最高资格、关闭原因 | Prediction 原字段保留开场真值，`episode` 保存 ID / 时点 / 上下屏障 / 最新概率与 feature ID / 最高资格 / close_reason，SymbolState.evaluation 保存当前完整量化输出 |
| TARGET / STOP / TIMEOUT 自动、确定性关闭 | 原首次触达顺序保留；7 个固定录制结果在线 / 离线一致；结果一次生成，不依赖模式、网页或订单 |
| SIGNAL_INVALIDATED / DATA_INVALIDATED 分离 | 稳定信号改变与数据健康异常分别关闭；原模型切换标记 SIGNAL_INVALIDATED；重启标记 DATA_INVALIDATED |
| MFE / MAE、终止收益及到期收益 | 首次结算真值保留；独立 HorizonObservation 跟踪至原期限；后续观察不覆盖首次触达；缺数据不伪造收益 |
| OBSERVE / skipped CONFIRM 仍产生学习数据 | Outcome 由行情引擎生成，执行与用户 Skip 不影响其闭合 |
| 预测与交易结果分别查询 | `outcome` / `outcome_horizon` 与 PAPER `trade` / 交易所 `live_trade` 分离；历史查询合并注释与到期观察 |
| 闭合 Episode 是统计单位 | 健康 / 校准 / 训练读取唯一闭合 Outcome；原训练按 prediction_id 去重；Evaluation 不写入预测训练集 |
| UI 稳定、已结束显示 Result / History | 当前概率与开场概率分别显示、Episode ID 固定；关闭后 active 清空，展示上次结果；预测统计、评估统计与交易数分开 |
| 重启不重复已关闭结果 | checkpoint 默认字段兼容；重复 restart 不重复终止记录；重启后后续 HorizonObservation 明确失效 |

评估政策：DATA_INVALIDATED 不进入胜负 / 校准分母；SIGNAL_INVALIDATED 属于在原 Triple Barrier 窗口结束前的截尾样本，也不当成“超时”或“预测错误”加入三分类训练。它保留在结果历史及完整性计数中，后续到期观察独立保存。新计数自升级开始，不伪造旧数据的累计次数。`episode.first_hit / first_hit_at` 仅屏障触达时有值；旧 `Outcome.first_hit_ts` 为兼容原数据保留，超时 / 失效时表示标签终止时点。

## v1.0.3 逐项对应

| 要求 | 实现与验证 |
|---|---|
| QUALIFIED 不等于 AUTO 执行 | 独立 admission.rs；SKIPPED 不改写原 Qualification 或预测真值 |
| AUTO ENTRY 必须 ADMITTED | 意图创建和 SUBMITTING 两处独立验证；缺少记录即拒绝；每个订单保存 admission 快照；RejectedCommand 持久化拒绝后的评估 / 审计状态，可确定性回放 |
| 独立且版本化 AUTO 净优势门槛 | 配置验证 `auto_min_net_edge >= min_net_edge`；默认 0.0004；旧配置迁移不降低原门槛 |
| 最新 Evaluation 发送前复核 | 命令持锁刷新概率 / Qualification，重新排序与预算检查；新准入和风控通过落盘后才提交网络请求 |
| trust / execution / risk hard fail | 分别有专项拒绝测试；账户、成本、流动性、过期、并发预留和原共享 Risk Gate 继续生效 |
| 竞争机会透明确定性选择 | priority-v1 tuple + 稳定 ID 最后比较；高优先级机会先预留名义金额 / 最大损失 / 并发槽位；低资格候选不抢预算 |
| 独立新 Edge、cooldown、churn | 退出后的信号 epoch、60 秒冷却、重新入场成本覆盖、窗口次数上限分别检查 |
| daily cap 只作上限 | 达上限只产生 SKIPPED；未达上限不会降低优势门槛或生成订单；保护性退出不受入场次数限制 |
| 完整审计链 | Episode → Qualification → auto_admission → live_risk_decision → intent / Plan → live_risk_recheck → broker / Trade；记录竞争者、前后预算、配置版本、原因 |
| 先在 PAPER / shadow 验证 | OBSERVE / PAPER / CONFIRM 生成不下单的 SHADOW 准入；专项测试覆盖选择与拒绝；真实 AUTO 仍需原有 PAPER 多窗口证据 |
| UI 与 AI 边界 | Card 独立显示 ADMITTED / SKIPPED 及主要 reason；审计可查询；解释服务没有写入准入或交易状态的路径 |

参数与账户边界见 [EXECUTION.md](EXECUTION.md)。默认策略为保守起点，不代表研究已证明最佳参数；可在风险设置中版本化修改，未知 priority policy 与 LOW 准入信任等级会拒绝。

## 执行检查

- Rust：78 项领域、Episode、Admission、执行与 Shadow 测试通过；包含 Binance / OKX 本机模拟接口。
- Python：12 项研究、时间隔离、GBT、Parquet 与录制质量测试通过。
- Rustfmt、Clippy（警告视为错误）、TypeScript、Next.js 生产构建通过。
- 固定 240 秒录制：3,122 条记录、7 个闭合 Episode、0 笔交易。fixture 直接计算与离线 replay 均为 `b42f6f9b27b4a6bf6f9960036995a1016828de673bc552023c4438e8ed2f5306`。原 11 个结果基准保存在 v0.2 验收；本次补齐实盘反馈与行情背景字段后，直接计算和离线重放一致，重新冻结序列化摘要。
- 浏览器：14 项测试通过，含 AUTO 独立展示、开场 / 最新概率、计数、持久化审计查询、配置门槛、桌面 / 手机及旧功能。
- 旧数据恢复、Binance / OKX 两标的公共行情就绪、OBSERVE、存储健康、无执行凭证 / 订单等升级检查见 [upgrade-smoke.json](upgrade-smoke.json)；本轮四项补齐后的进程恢复与公共行情检查见 [completion-smoke.json](completion-smoke.json)。

本次为功能与自动化验证交付。没有配置 / 使用真实交易凭证，没有发出真实资金订单；交易所模拟账户 / 小额实盘联调、持续运行表现与真实策略盈利门槛仍待相应数据验收。既有 REST 轮询、本地止损及电脑运行条件的限制不因本次升级消失。


## 原文档差距补齐（四项）

| 本轮范围 | 已实现 | 验证证据 |
|---|---|---|
| 保护性实盘执行不依赖慢账户请求 | 独立账户、规则、每标的增仓 / 减仓 / 撤单任务；确认成交量退出，保护状态锁定；拒绝重试间隔；UNKNOWN 不重发；写盘后核对授权 | 卡住账户的模拟网关仍执行保护和撤单；检查网络提交前 SUBMITTING 已写入日志；kill 下排队新入场被取消；部分成交、晚到成交、重复 / 拒绝退出专项测试 |
| 实盘结果反馈模型健康 | 冻结参考价 / 版本 / 成本；ENTRY、独立 ADD、分批退出聚合；单列实盘成本 / EV 偏差，异常降低信任；后续资金费修正 | 成本及净收益数值断言、资金费调整只保留一条交易、样本 / 版本过滤、LOW 不被提升 |
| 实盘持仓风险展示 | 服务端风险投影；确认量与账户量、快照年龄、当前 / 目标敞口及杠杆、止盈止损、动作原因、待核实状态和实盘反馈 | 后端保护目标为零；浏览器合成持仓验证过期账户仍显示待平确认量，入场禁用、平仓可用，手机布局不溢出 |
| 现有跨市场特征计算与验证 | 一秒收益差按同一时点取样；60 秒 / 500ms 网格的双向领先滞后相关性，只作行情背景 | 已知 Binance / OKX 各自领先与同步、未来数据不影响过去、过期 / 稀疏 / 平价 / 缺数据不可用 |

本轮未扩 OI、强平流、复杂模型或 UI 重构。领先滞后基于本机接收时间，会受到网络差异影响；原十维模型输入保持不变。旧实盘记录没有冻结参考时不进入新反馈统计。

代码与本地回归已补齐这四项；交易所模拟账户 / 小额实盘验收仍没有完成，本轮不把模拟网关证据当成实盘运行结果。本机停止 / 休眠和交易所不可达时的退出限制仍存在。
