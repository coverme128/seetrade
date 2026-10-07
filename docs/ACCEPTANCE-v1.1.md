# v1.1 实现与验收

当前程序增量为 0.5.2，每日复盘与效果评估见 [0.5.2 验收](ACCEPTANCE-v0.5.2.md)。下文保留 0.5.1 修复时的验证记录。

程序版本：0.5.1。对应升级包：`micro-alpha-trading-v1.1-upgrade-for-codex-v1.0.4-base.zip`。
包 SHA-256：`b897d474123245ef1ee18fb4461af6679eaa3e8ab809a2c6bfc994ce2ced20ae`。
26 份原始文件保存在 [spec-v1.1](spec-v1.1/UPGRADE-CONTRACT.md)，不修改源合同。

修复结论（2026-10-04，0.5.1）：**本次复核发现的正确性、版本隔离、追溯和展示问题已修复，个人范围的工程回归通过。**
保留有版本的固定资格规则和滑点探针金额，未实现任意规则编辑。文末保留修复前问题及逐项处理结果；尚未取得多资产长期 OBSERVE/PAPER 或受控实盘就绪证据。

## 逐项实现

| 合同范围 | 当前实现 | 主要证据 |
|---|---|---|
| 公共 Universe / Scanner | Binance、OKX USDT 永续元数据；手动、定时、定向添加共用路径；有界 L2 采样；未知指标与部分失败显式呈现 | `scanner.rs`、公共 SOL 定向扫描、解析测试 |
| 资产资格与生命周期 | 研究优先级与交易资格分离；容量、预热、连续性、软降级和硬暂停；新增币种不能直接晋级或获得 AUTO | `universe.rs`、生命周期/权限测试 |
| 版本化档案 | 每资产独立配置、模型/校准引用、触发分布、成本/风险和流动性证据引用；H/T/S 候选；按交易所成本覆盖 | 档案隔离测试、数据库投影与恢复测试 |
| 动态共享引擎 | 动态订阅和退出；交易所集合变化时重建订阅；相同特征、资格、首次触达及 Outcome；小价格合约精度 | SOL Episode→TARGET_HIT 测试、小价格测试、双黄金回放 |
| 币种研究 | 按币种隔离训练；按来源/配置/H-T-S 选择数据；24 组固定边界回放；独立影子比较、手动晋升/回滚；定时队列 | Rust grid 测试、Python 资产数据隔离测试、原 shadow 测试 |
| 触发运行时继承 | IDLE 轻量 Pulse；四族因果 robust-z 校准；活跃 Episode 和持仓继续复评；人工参数不冒充已校准分布 | runtime_trigger 测试、资产触发校准测试、回放对比 |
| Scanner / Radar 界面 | 排序、筛选、分页、详情、未知数据、状态变迁；雷达只聚合正式活跃 Episode；颜色等级和原预测详情跳转 | Playwright 桌面/手机测试 |
| 组合风险 | 持仓、待成交订单、风险/方向/相关性/币种/交易所上限；PASS/REDUCE/BLOCK；不增大单笔建议 | portfolio 与 live 集成测试 |
| 实盘兼容 | 新币默认禁用实盘；继承 AUTO Admission；组合决策绑定意图；发送前复核；降低风险订单保持可用 | 提交前 veto 测试和原 live/main 执行测试 |
| 存储与回退 | 追加迁移、注册表、扫描观测、档案、资格、状态变迁、组合决策审计；冻结动态池保留仓位保护和历史 | SQLite 幂等/外键/恢复测试、迁移证据 |

## 自动化验证

- Rust：118 项测试通过；相对首次 v1.1 交付新增 7 项针对本次缺口的回归。
- Rust：`cargo fmt --all -- --check`、`cargo clippy --locked --all-targets -- -D warnings`。
- Python：16 项研究/数据测试通过。
- 网页：23 项 Playwright 测试验证通过（完整运行 22 项通过，冻结行情测试改用可控时钟后单项复跑通过）。已完成预测卡测试改用固定合成行情结果，消除对新实例运行时长的依赖。
- 网页：TypeScript、Prettier、生产构建。
- 原始 BTC/ETH 黄金回放保留；动态 Universe 未启用的参考夹具没有改变摘要。
- 触发回放：两条有效参考 Episode 均匹配，完整评估从 480 次降至 54 次；该固定夹具不代表所有实时市场机会的召回率。

## 公共网络与迁移证据

[公共定向扫描](evidence-v1.1/public-targeted-scan.json)：双来源成功，扫描当时发现 879 个规范化候选；SOL 的公开深度和滑点可观测。尚无足够本地连续采样的波动、尾部、价差分布保持未知。数量和行情随时间变化。

[迁移前检查](evidence-v1.1/migration-before.json)：原实例为 OBSERVE、配置版本 35、模型 `score-v1-unvalidated`，无账户持仓或待成交订单。升级前通过 SQLite backup API 保存 `data/backups/v1.0.4-before-v1.1.sqlite`，完整性检查通过。备份属于本机运行数据，不纳入 Git。

交易所接口依据：[Binance USDⓈ-M 公共市场 API](https://developers.binance.info/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/rest-api/market-data)、[OKX API v5](https://www.okx.com/docs-v5/en/)。公共市场验证不需要账号或 API Key。

## 验收边界与个人使用

1. 默认 OBSERVE、定时扫描关闭、组合实盘关闭。扫描 HIGH 是研究排序，不是交易指令。
2. 本轮没有用真实资金下单，没有以缩短预热、伪造 Outcome 或降低 Model Trust 代替长期验证。
3. 新币默认需要 24 小时连续健康数据、2,000 条同模型/配置有效 Outcome、持续资格和模型/触发校准证据。真实多币 OBSERVE/PAPER 和小额实盘验收必须在后续采集中取得。
4. 12 个采集槽位是配置上限；本轮短时连通性检查不能证明 12 币长期吞吐和完整交易所故障覆盖。
5. 单场所币种可进入发现/采集，但原双场所交易特征要求不放宽。个人执行仍是一次选择一个交易所账户，未加入跨账户自动分配。
6. 研究队列一次运行一个任务；网格报告属于探索证据，不能替代新的时序样本外检验。
7. 保留原首页与预测页，增加 Scanner/Radar 和组合摘要；没有扩展 OI、强平流、新闻、复杂模型或重做 UI。

日常操作、配置和回退见 [动态币种池指南](UNIVERSE-GUIDE.md)。

## 本机升级结果

### v1.0.4 → 0.5.0：首次动态池迁移

首次升级的 [迁移后证据](evidence-v1.1/migration-after.json)确认配置版本 35、原模型、观察模式均保留。最终备份有 45,370 条记录，升级后逐行集合比较：旧记录缺失或变化为 **0**；SQLite 完整性通过，无外键错误。额外最终备份为 `data/backups/v1.0.4-final-before-v1.1.sqlite`。

[隔离公共行情验收](evidence-v1.1/runtime-public-smoke.json)记录 BTC/ETH/SOL 三币双场所采集。SOL 初次 Binance DNS 解析失败，自动重连后六路行情均 ready；全程无私有账户，仅 OBSERVE，新增 SOL 保持 COLLECTING。该隔离实例已停止，原实例没有被加入测试 SOL。

### 0.5.0 → 0.5.1：本次修复升级

2026-10-04 升级检查时，原服务使用 0.5.1 核心与生产网页，入口仍为 `http://127.0.0.1:3000`。[本次升级证据](evidence-v1.1/fix-upgrade.json) 对应备份 `data/backups/v0.5.0-before-v0.5.1.sqlite`：备份有 51,280 条记录，检查时升级库有 51,318 条，旧记录缺失或变化为 **0**；SQLite 完整性及外键检查通过。配置版本 35、原模型及 OBSERVE 保持不变，扫描与组合实盘仍关闭。

生产网页首页、Scanner、Radar 加载正常，浏览器无页面错误。启动时 DNS / 请求失败经自动重连恢复，最终检查 BTC/ETH 的 Binance、OKX 四路均 ready。这是升级时的采样记录，不表示持续可用性已经验收；未修改网络设置或实盘权限。

[验证摘要与源文档指纹](evidence-v1.1/validation.json)、[Scanner 截图](screenshots/universe-scanner.png)、[Radar 截图](screenshots/universe-radar.png)。


## 2026-10-04 代码复核发现（修复前记录）

以下保存修复前的发现、当时的测试数量及代码行号；它们不代表当前缺口或当前行号。处理结果见文末“0.5.1 修复验证”。

本次将源 ZIP 的 26 个文档与归档逐字节核对，全部一致。审查覆盖 scanner、生命周期、模型/研究、PAPER/live 执行、组合门禁、存储及界面。仅新增隔离诊断示例与复核证据，没有修改业务代码或运行实例。以下 P0/P1/P2 是本次修复优先级，不是原合同另外规定的新功能。

### P0-1：独立币种参数未贯穿执行和持仓管理

- `engine/src/live.rs:692` 的限价边界及 risk_cost 仍读取全局 `self.config.max_slippage`；而 Trade Qualification 已读取资产配置。
- `engine/src/core.rs:1853`、`1931` 的 PAPER 加减仓间隔、死区、独立信号确认、次数、风险预算等仍取全局配置。live 的加减仓和反馈阈值也存在同类读取。
- 影响：为某个币种配置更严格的限制，不能保证在后续执行阶段按同一配置生效。需要区分明确的账户级限制与币种级限制，统一后者的解析，并补充更严格币种限额的端到端回归。
- 对应合同：ASSET-PROFILE-CONTRACT 的资产特定风险、成本和兼容配置规则。

### P0-2：成本与反馈上下文不一致

- `core.rs:830` 的预测成本使用 `venue_costs`，但 `core.rs:1385` 的冻结 `cost_assumptions_bps` 仍用基础字段；`core.rs:1703` 的 PAPER 成交成本同样直接用基础字段。研究 `train.py` 和 `ShadowRun::prepare` 又使用旧成本口径。
- 隔离复现：Binance 覆盖设置单边手续费 8 bps、往返假设合计 25.7 bps，预测 qualification 确实记录 8 bps 单边手续费，但供研究使用的冻结成本仅 11.5 bps。见 [audit-probe.json](evidence-v1.1/audit-probe.json)。
- `core.rs:1212` 把模拟成交的资产配置与全局配置比较，资产有独立配置时，相应成交会被排除在健康反馈样本之外。
- 影响：成本、净 EV 的研究解释和模拟反馈不能视为完整闭环。需统一预测、研究、影子、模拟成交及实盘反馈的资产/场所成本上下文。

### P1-1：配置版本尚未按资产隔离

- `universe.rs:815` 修改任意资产配置都会推进全局 config_version；其他资产 Outcome 筛选（`core.rs:1203`）、研究数据选择（`research/train.py`）和晋级计数都依赖该全局版本。
- 隔离复现：只改 ETH 风险配置，BTC 配置对象没有变化，但全局版本从 1 变为 2。
- 影响：其他币种的既有样本不再进入“当前版本”统计，可能退回样本不足并重新累计。应将账户级配置版本与资产配置版本分别绑定到预测、研究和晋级依据。

### P1-2：档案与生命周期审计不完整

- `core.rs:399` 全局模型晋升直接修改 BTC/ETH 档案内模型，却没有调用 save_profile、递增档案版本或同步校准引用；全局模型回退也有同类路径。可导致同一档案 ID 在运行状态与追加存储中代表不同模型。
- `universe.rs:243` 的 Transition 没有统一规则/配置版本字段，部分 transition evidence 为空。已有日志可以辅助追溯，但不满足合同“每条变迁绑定证据与规则版本”的完整要求。
- AssetProfile 没有显式 model_trust_state；运行态另有 health.trust，需明确两者的版本关系，避免只补一个会过时的字符串字段。

### P1-3：资产资格规则存在遗漏

- `scanner.rs` 会统计 resets，但 `universe.rs::qualify` 没有把 book stability / reset rate 作为正式资格维度。
- 资格阈值可配置，但哪些维度为硬 FAIL 仍由 qualify 内部布尔常量决定；滑点测试名义金额固定为 100/500/1000，未成为可配置且版本化的规则。
- 实时订单簿完整性和 Trade Qualification 仍提供保护；此发现不等于已证明可以绕过全部下单门禁，但资产资格层尚未完整对齐合同。

### P2：界面与展示补齐

- Scanner 档案详情尚未完整呈现 Model Trust 和流动性档案引用，虽然运行态/后台部分数据已存在。
- 组合摘要 positions 使用敞口行数（`portfolio.rs:85`）；同币种仓位加待成交订单可能显示为多个仓位。应分开显示去重持仓币种与待成交订单。
- 机会雷达、颜色、排序和详情跳转已实现；这些展示问题不应与核心链路尚未实现混为一谈。

### 验证证据仍待取得

多币 24 小时以上连续 OBSERVE、达到要求的同上下文 Outcome、实际多币 PAPER 预期/实现成本与 EV 对照、12 槽位持续压力、受控小额真实执行证据尚未完成。首次 0.5.0 交付的 111/15/23 项测试和三币短时公开行情连通性证明已有实现可运行，不能替代这些验收。

复核时确定先处理 P0-1、P0-2，再修复 P1 的版本 / 审计和资格规则、补齐 P2；上述工程修复现已完成，结果如下。后续仍需按文档取得长期 OBSERVE/PAPER 与受控实盘证据。


## 0.5.1 修复验证

| 复核项 | 处理结果 | 回归证据 |
|---|---|---|
| P0-1 独立限制未贯穿 | 统一执行滑点、时效、加减仓与反馈阈值；AUTO 同时检查账户/币种门槛 | `independent_asset_slippage_is_used_at_live_intent_boundary`、`paper_uses_frozen_asset_rebalance_limits_and_venue_fees` |
| P0-2 成本与反馈不一致 | 解析场所成本后统一预测冻结、研究、影子、模拟成交；按有效资产配置筛选反馈 | `venue_costs_are_frozen_in_prediction_research_context`、`independent_asset_paper_fills_remain_in_model_feedback` |
| P1-1 配置版本互相影响 | 独立资产版本；全局配置变更才使全部资产上下文改变；网页使用对应资产版本 | Rust 独立版本恢复测试、Python 跨币种研究选择测试、网页机会版本测试 |
| P1-2 审计缺口 | 共享模型变更追加档案；独立配置不被覆盖；生命周期绑定规则/配置快照 | `shared_model_rollback_versions_profiles_without_overwriting_independent_asset`、transition 断言 |
| P1-3 资格维度遗漏 | 新增最近一小时重置/重连次数硬失败规则；固定策略有版本 | `excessive_recent_resets_are_auditable_hard_qualification_failure` |
| P2 展示 | 档案展示当前 Trust、流动性引用；占用币种去重 | 网页回归、类型检查 |

本次按个人使用范围保留固定的硬失败分类和滑点探针金额，不增加任意规则编辑器；这是经用户讨论收缩后的实现边界，不能称为原文“全部维度任意配置”已经实现。Model Trust 引用已有运行态，不新增冗余状态系统。

修复前 [audit-probe.json](evidence-v1.1/audit-probe.json) 与修复后 [audit-after.json](evidence-v1.1/audit-after.json) 对照：独立 ETH 配置不再改变 BTC 的全局版本；配置 25.7 bps 与预测冻结的研究成本均为 25.7 bps。原始历史成本记录保持原样，不倒改数据。

验收仍区分工程回归与真实市场验证。多币长期 OBSERVE/PAPER、12 槽位持续负载及受控小额实盘证据尚未取得。


升级操作和旧成本样本的处理见 [0.5.1 升级检查](UNIVERSE-GUIDE.md#升级到-051-后的检查)；本文引用既有验证证据，本次文档更新未重新执行测试或采集新的市场证据。
