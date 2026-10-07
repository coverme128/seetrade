# 交付与验收 · v0.4 / 合同 v1.0.4 · 2026-10-03

最新增量交付为程序 0.5.2 / 合同 v1.1，见 [0.5.2 增量验收](ACCEPTANCE-v0.5.2.md)；动态池基线见 [v1.1 验收记录](ACCEPTANCE-v1.1.md)。下文保留 v1.0.4 的原始验收证据。


使用和运维入口：[运行时指南](RUNTIME-GUIDE.md)、[运行配置](EXECUTION.md)、[运维手册](OPERATIONS.md)。以下结果为 2026-10-03 已保存的验收记录，不表示每次文档更新都重新执行了测试或实时采样。

## v1.0.4 Runtime Trigger 升级

原始升级包的 15 份合同和 Manifest 已逐文件校验 SHA-256，并原样保存在 [spec-v1.0.4](spec-v1.0.4/UPGRADE-CONTRACT.md)。ZIP SHA-256：`ff1e89ff2a976ca07183e7678c5e4acdad526b888353581b63cedc68109f7c1f`。本节描述运行时增量；下方 v1.0.2 / v1.0.3 表格保留为历史验收记录，其“所有标的每秒完整评估”调度已由本节替代。

### 六批次对应

| 批次 | 实现 |
|---|---|
| 01 Market Pulse / 状态 | `market_pulse.rs`、`trigger_policy.rs`、`core.rs`；连续维护规范簿与成交，IDLE 只做轻量检测，IDLE / CANDIDATE / ACTIVE / IN_POSITION 与 Episode 生命周期分离 |
| 02 四类 Alpha Trigger | 订单流、主动成交、盘口压力 / 深度、价格速度；逐标的历史 robust-z；NORMAL / ABNORMAL / EXTREME；任一 EXTREME 或同窗两个不同 ABNORMAL 才唤醒 |
| 03 评估节奏 | 有效触发立即完整评估；候选默认 1.5 秒 / 10 秒 TTL，Episode 和持仓默认 1 秒；同一上下文保留 trigger ID，更强证据只升级上下文；结束后保留既有 signal epoch，避免同向重复发布 |
| 04 Risk / Data 保护 | 风险恶化、账户异常、断档 / 坏簿 / 断连 / 过期 / 时间偏差可即时打断 cadence；当前核心的 Qualification、Position、Live Exit 决定后续动作，DATA 立即清除可执行资格 |
| 05 漏斗 / 回放对照 | `trigger_analytics.rs` 保存关联；`/api/trigger-analytics`、历史 API 与折叠诊断面板可查询；`trigger_compare` 离线比较事件驱动与固定步进，报告潜在漏机会 |
| 06 迁移 / 回归 / 运行 | 旧状态默认字段恢复，原历史不重写；旧合同测试用固定步进研究夹具保留控制实验，新增生产事件调度、门槛、账户异常即时保护与比较测试；实际结果见本节验收证据 |

### 实际指标与工程默认值

Pulse 只使用 Binance 主场所的规范盘口与成交，OKX 仍参加完整特征、资格与数据保护。四类指标每标的独立，归一化只读取当前采样之前的历史：

| 家族 | 原始轻量指标 |
|---|---|
| ORDER_FLOW_CHANGE | 簿内递推 OFI 的绝对值、相邻 Pulse 的 OFI 变化绝对值 |
| AGGRESSIVE_TRADE_BURST | 最近 1 秒有符号成交名义金额 / 总名义金额的绝对值、`ln(1 + 总成交名义金额)`；Pulse 窗口最多保留 4,096 笔 |
| BOOK_IMBALANCE_DEPTH | 前 20 档 bid / ask 名义深度失衡绝对值、相邻采样总深度下降比例 |
| PRICE_VELOCITY | 相邻采样中间价收益 / 实际秒数的绝对值、速度变化绝对值 |

对每个指标计算 `max(0, (x - median(history)) / max(1.4826 × MAD, 0.05 × abs(median), 1e-9))`，上限 1,000。每个家族取其两个归一化指标最大值。MAD 与尺度下限来自该资产自己的历史；`1e-9` 只是避免除零，不是统一价格涨幅触发线。

默认配置：方法 `robust_z_v1`，采样间隔 250 ms，滚动窗口 120 秒，预热 32 个历史样本，ABNORMAL 为 z ≥ 3，EXTREME 为 z ≥ 6，同窗 1,000 ms，debounce 2,000 ms，候选 TTL 10,000 ms / cadence 1,500 ms，ACTIVE / IN_POSITION cadence 均 1,000 ms。各家族和风险保护默认启用；额外相对深度塌陷为下降 50%，同样可版本化配置。它们是观察 / 回放起点，不构成有效 Alpha 或最优阈值结论。

候选 / Episode 未结束时，重复相同触发不新建业务对象；更强家族 / 严重度可更新同一 ID 并立即评估。上下文结束后，同一持续异常需先回归非组合触发状态，才能再次触发。Pulse tick 不单独落为业务记录，原行情仍连续录制。Data 异常清除受影响的基线并重新预热；新鲜 snapshot 等待首个 bridge delta 期间禁止新计划，超时 / 断档继续 fail-closed。

风险触发沿用已有价差、深度、最大滑点、波动和止损预算。滑点触发量是“半价差 + 配置滑点 + 冲击”的代理，实际执行漂移仍由原有 Model Health / Live Feedback 处理。真实持仓触及原 TP / SL、账户查询异常也直接进入原风险路径；不会等待下一次 Alpha Clock，更不会等待账户查询完成才允许运行独立减仓任务。

### 分析口径与使用入口

`market_pulse_trigger`、`trigger_evaluation`、`trigger_candidate_closed`、`protection_trigger`、`protection_result` 与已有 prediction / qualification / admission / execution / outcome / trade 一起持久化。Episode 添加可选的 `origin_trigger_id` / `origin_trigger_families`；旧历史缺失时保持缺失，不补造。Trigger 事件保存实际交换所 / 接收时点、严重度、归一化快照、配置版本和调度前状态。

分析按家族及组合分组，提供 trigger / full evaluation / episode / qualified / AUTO admitted / executed 数量、结果分布、有效 Outcome 的观察净优势、PAPER 与 Live 实际净损益及无 Episode 候选率。只有 LIVE 上下文的 ADMITTED 才计入 AUTO 准入；ACK 或提交意图不计执行，需确认非零成交。资金费调整覆盖同一 Live Trade，不重复增加笔数。

内存 / API 为最近最多 2,000 个已结束触发加未结束关联，报告注明范围与淘汰数，不冒充全历史累计。全部原始事件仍在 SQLite / JSONL。分母明确：Episode / Trigger；Qualified / Episode；Admitted / Qualified；Executed / Episode；已结束无 Episode 候选 / Trigger（未结束及数据中断不计无边际信号；重启 / 配置切换也不计）。

```bash
source scripts/env.sh
# 可重复的安静 → 已知突发 → 触达目标夹具
cargo run --locked --example trigger_fixture -- data/trigger-fixture.jsonl
# 独立离线对照，不提供生产运行模式切换
cargo run --locked --example trigger_compare -- data/trigger-fixture.jsonl
# 同样可以对完整真实录制执行研究对照
cargo run --locked --example trigger_compare -- data/live-validation/live-SESSION.jsonl
./scripts/replay-check.sh
curl -s http://127.0.0.1:8787/api/trigger-analytics
```

对照从录制起点的冻结模型 / 配置冷启动，两路均为 OBSERVE；仅沿用风险 / 触发配置命令，忽略且计数模型晋升、账户、交易等控制记录。因此它是策略研究重算，不是订单账本原样回放。候选匹配采取相同 symbol / direction / config、开场时间相差不超过候选 TTL 的一对一匹配。“潜在有用”参考 Episode 必须有效闭合，且曾 QUALIFIED，或观察净优势与开场预期净优势均为正。末尾未闭合结果截尾，不伪造 timeout。Canonical 原样重放仍使用 `--replay`。

### 迁移、回退与证据边界

生产启动强制事件驱动；研究用固定步进标记不能从 checkpoint 恢复为生产模式，也不在界面 / 控制 API 暴露。旧 v1.0.3 状态首次升级会初始化触发配置版本 1 并推进全局配置版本，原模型、风险预算、账本、历史结果保留，新运行样本单独积累。重启仍回到 OBSERVE、重新建立行情簿，未确认订单仍需对账。旧 Journal 无触发版本时，离线 canonical replay 使用原固定步进政策；新字段改变序列化摘要，旧摘要保留为历史文件。

修改触发参数只允许 OBSERVE 且无持仓 / 待处理订单，校验范围、推进触发和全局配置版本、结束原候选 / Episode、暂停正在运行的 Shadow；不会自动改生产阈值。界面只在原有折叠技术区域补充配置、来源和漏斗，不新增扫描器、交易所、标的或主卡片。

回退需先停止实例，在无未完成真实订单 / 持仓时切回原 v1.0.3 部署和对应备份；保留新增触发日志。已有真实执行状态先对账，不能用旧 checkpoint 回滚成交事实。固定步进回退是部署操作，不是用户常驻开关。


### 本轮回归与可重复证据

- Rust 96 项通过（原 79 项 + 16 项运行时测试 + 1 项真实执行保护集成测试）。原领域 / 执行 / Episode / Admission / Shadow 夹具显式使用固定步进参考构造，保留 v1.0.3 控制实验；新增测试直接使用生产事件驱动构造，覆盖静默、四家族、组合、候选过期、同向去重、门槛、数据失败、持仓保护、迁移和确定性。
- Rustfmt、Clippy `-D warnings`、TypeScript、Next.js 生产构建通过；Python 14 项研究回归通过。
- 浏览器 15 项通过，含新折叠诊断 / 版本化配置、旧预测结果和 AUTO 展示、手机布局。测试使用独立 `data/v104-browser` 合成实例，不修改真实采集的配置；首次启动须先等待 BTC 出现闭合结果，两个旧卡片测试依赖此数据前提。
- 原 240 秒固定步进夹具仍为 7 个闭合结果、0 交易；当前直接计算与 canonical replay 摘要均为 `364418f87d3bc3f7c02f37daef0af7cfcb331437153c5b7391a22cf0f4bd5832`。v0.3 摘要保存在 `fixtures/demo-240-v0.3.sha256`，新序列化字段引起摘要变化。
- 新 240 秒夹具：12,480 个输入事件、54 次完整评估、2 个 Episode、2 个 TARGET_HIT、0 交易；直接与 canonical 摘要均为 `74fd883c2ffc98c49c3688ef6600260541d344045db69d9eab79f2918d93ffe5`。参考路径为 480 次评估，同样 2 个 Episode / Outcome；评估减少 88.75%，两次潜在有效机会一对一匹配，未重复发布。详见 [回放证据](runtime-trigger-replay.json)。
- 封存真实录制 `live-1790927814755.jsonl`：30,049 个事件；事件驱动 221 次评估、11 个 Episode，参考 300 次评估、13 个 Episode，减少 26.33%。参考有效机会分母为 0，因此**不能据此计算或宣称真实漏机会率为零**。4 条非配置控制记录被显式排除；末尾未闭合 Episode 截尾。详见 [真实录制研究对照](runtime-trigger-real-comparison.json)。
- 真实旧会话起点包含已填充的整数价格键订单簿，曾暴露 Journal tagged-enum 反序列化问题；已保留格式、修复起点解析并新增非空簿往返测试，真实封存对照跑通。

### G1–G6 门禁结果

| 门禁 | 结果与依据 |
|---|---|
| G1 IDLE_EFFICIENCY | PASS；5 分钟静默夹具持续维护健康簿和图表，完整评估为 0；真实回放也减少评估 |
| G2 TRIGGER_DETERMINISM | PASS；四家族分级、组合、预热、去重与配置测试；两套 canonical 黄金摘要一致 |
| G3 EPISODE_COMPATIBILITY | PASS；连续机会仅一个 Episode，未交易结果照常闭合；旧 7 结果基线保留，新夹具 2 个目标结果 |
| G4 PROTECTION | PASS（代码 / 模拟网关）；全部文档数据与风险注入；确认部分成交后账户异常无需等 Clock 即生成独立 reduce-only 退出；旧慢账户网关保护回归通过 |
| G5 AUTO_REGRESSION | PASS（代码 / 模拟网关）；保留资格、成本、LOW、准入、发送前复核与去重；触发不能越过原门槛 |
| G6 TRIGGER_ANALYTICS | PASS；来源链、漏斗、确认成交和资金费去重；固定步进对照；全部关闭触发的反向夹具准确报告两次漏机会 |

诊断界面仍位于原折叠区域，见 [运行触发截图](screenshots/runtime-trigger.png)。G4 / G5 的自动测试不等价于交易所私有模拟账户或真实资金验收。

### G7 持续运行结果与交付边界

**G7：本机 release / OBSERVE 的 5 分钟限定窗口 PASS；长期及私有账户运行不在本轮验收范围内。** 原 `data/live-validation` 升级恢复，风险配置版本从 34 推进到 35，触发配置版本 1。61 次采样均为四路公共行情就绪、存储健康、OBSERVE、无执行凭证 / 持仓 / 订单；请求失败 0，核心状态最大滞后 121 ms。RSS 为 71,420–94,888 KB，采样 CPU 为 1.6–42.1%（`ps` 口径）。此窗口内 BTC / ETH 完整评估分别 252 / 244 次，触发事件 53 条，任一秒最多 2 条；16 个 Episode ID 无重复。

最后一次采样的完整评估 P95 为 588 µs，Pulse P95 为 149 µs；checkpoint 序列化 P95 为 18,604 µs，输入队列最近窗口最大延迟 10 ms。耗时分布是本机近期窗口，不能推导成交延迟或资金安全保证。原始采样与持久化审计统计见 [优化构建运行证据](runtime-trigger-smoke.json)。

开发构建的对照观察在并行编译时出现最高 55053 ms 状态积压，**不通过运行就绪判断**；记录保存在 [开发构建压力证据](runtime-trigger-debug-stress.json)，没有用失败样本替换成功证据。个人持续运行新增 `./scripts/start.sh`，使用优化核心与生产前端；`dev.sh` 仅作开发调试。此次优化没有放松订单持久化、发送前数据新鲜度或风险门槛。

实现范围已覆盖升级包六批次，G1–G6 的功能 / 回归门禁及上述限定 G7 观察通过。真实模型仍需校准与当前版本证据，交易所私有模拟账户、小额实盘和长期稳定性仍待独立验收；本轮没有使用私有凭证或真实资金订单。

### 合同自查

没有增加主卡片、模型家族、交易所、标的、OI 或强平流。四类 Alpha 只决定何时评估；模型、资格、AUTO 准入、风控和执行仍有各自门槛。当前阈值是合成夹具和有限真实录制可复现的初始基线，并非统计优化结果。建议每次准备调整阈值、出现市场状态变化或积累新一批封存录制时，重复 `trigger_compare`，保留对照报告后再人工调整；没有自动参数改写或模型晋升。

## 机会等级与排序追加验收 · 2026-10-03

首页与预测记录页加入绿 / 蓝 / 黄 / 红 / 灰五类等级、双标的当前机会提示、开场等级与净优势列，以及等级优先 / 时间最新排序。绿色依据现有资格全部通过且模型信任 HIGH；蓝色为资格通过且 DEGRADED。未校准、高原始概率、最终目标命中都不能独立提高等级。当前机会在断连、过期、配置变化、模型失配、kill 或存储异常时撤下优先高亮；历史只读取冻结开场资格。

本次仅修改展示与只读排序，未改变 Rust 交易、准入或风险逻辑，也未保存个人运行配置。新增 5 项分级 / 排序 / 过期及跨页浏览器测试，另运行 5 项相关旧界面只读回归，共 10 项通过；这不表示本轮重跑了全部 Rust / Python 或私有账户验收。前端类型与生产构建检查结果在交付时核对。

展示截图使用明确标记的合成状态：[首页](screenshots/opportunity-overview.png)、[预测记录](screenshots/opportunity-history.png)、[手机](screenshots/opportunity-mobile.png)。它们演示通过与拒绝的展示规则，不是当前真实市场存在合格机会的证据。使用规则见 [颜色与排序](RUNTIME-GUIDE.md#机会颜色与排序)。

## v1.0.3 历史基线

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
