# 交付与验收记录 · 2026-10-02

交付版本：**Micro Alpha Personal v0.1 — OBSERVE / PAPER**。参考合同：v1.0.1。个人使用、USDT 永续合约，默认 OBSERVE / AI OFF。本文区分功能验收与进入真钱交易的证据门槛。

## 已执行的检查

| 检查 | 实际结果 |
|---|---|
| Rust 领域 / 恢复测试 | **26 passed** |
| Python 离线研究 / Parquet 测试 | **6 passed** |
| Playwright 浏览器测试 | **4 passed** |
| `cargo clippy --all-targets -- -D warnings` | **PASS** |
| Next.js 生产构建及 TypeScript 检查 | **PASS** |
| 固定 240 秒行情回放 | **PASS**：3,122 条记录，11 条 Outcome，0 笔交易 |
| Parquet 实际导出与字段 round-trip | **PASS** |
| Binance / OKX 公共行情短测 | **PASS**：BTC、ETH 四组连接均收到深度与成交；不是长期稳定性验收 |
| 桌面 / 手机界面 | **PASS**：Chrome 实际交互，无页面脚本异常，390px 无整体横向溢出 |

固定回放的状态 SHA-256：

```text
ee3195355d5caac4c33d7835a5eae77be1203bb91bc86a731ddcaf2b18fc56f6
```

测试验证的是完整引擎状态，包括本地簿、特征、预测、资格、结果、配置和账户。固定录制没有满足可信度的交易机会，因此 0 笔模拟交易是预期结果；独立的合格信号测试验证了自动模拟入场和完整审计链。

真实行情短测证据保存于 [live-smoke.json](live-smoke.json)。取样时已记录 32 条正式预测、30 条自动结果，两交易所均连接，未检测到序列 gap；曾出现超过 3 秒的数据时间延迟，系统按规则使预测失效 / 阻止资格。测试期间无任何真钱订单，也未使用 API 密钥。

截图：[桌面](screenshots/workspace-desktop.png)、[手机](screenshots/workspace-mobile.png)。截图来自真实公共行情，数值随运行变化。手机截图中的历史表格可在表格内部横向滚动。

## 核心测试覆盖

- Binance snapshot 首次衔接、增量删除、pu 断档检测和恢复；OKX 前序序列匹配与序号重置。
- 陈旧 / 交叉 / 非法盘口失效，First-hit 只关闭一次，超时后的触达不能覆盖结果，跳空尾损不截断。
- 正向高概率但负 Net EV 拒绝；缺校准 / LOW Trust 拒绝；杠杆、流动性、日亏损、回撤与全账户暴露上限。
- 模型健康降级不改模型权重；演示模型不能加载到真实来源；错误配置不能修改已有状态。
- 模拟入场必须是新合格预测，完整 Prediction → Qualification → Plan → Execution 关联。
- 部分平仓保存剩余数量；数据过期不伪造成交；有效报价恢复后退出；亏损不加仓；kill 的 hold / flatten 行为明确。
- SQLite 提交确认、恢复风险状态、独立注释读取；重启自动回到 OBSERVE。
- 研究数据时间切分清除越界标签；校准降低过度自信；逻辑回归实际拟合；真实 / 演示数据严格隔离；完整报告生成。
- UI 交易对切换、模式切换、kill、配置持久化、健康 / 历史页面；跨来源控制拒绝，直接请求 AUTO 也拒绝。

## 合同阶段映射

| 阶段 | 已交付 | 状态 / 已知缺口 |
|---|---|---|
| P0.1 行情与回放 | 两交易所四标的连接、L2 / BBO / aggressor trades、Binance mark/index、序列检查、规范化日志、Parquet 导出与跟随录制、确定性回放 | 功能 PASS；OI / 清算流未接入，Parquet 为旁路进程 |
| P0.2 特征与标签 | 首波九类特征、跨市场差价 / 1s 领先差、三重屏障、MFE / MAE、自动 Outcome | PASS；领先差是简单代理，不是经过验证的因果 lead-lag 模型 |
| P0.3 Prediction / OBSERVE | 冻结评分、离线逻辑回归与温度校准、概率分离、推荐 / WHY NOW / 结果卡 | 功能 PASS；没有伪造已校准生产模型 |
| P0.4 Qualification | 13 项硬检查，成本和风险否决，完整原因码 | PASS |
| P1.5 Model Health | 分桶、ECE、Brier、近期 / 基线、Trust、四类偏移与重训建议 | 功能 PASS；真实样本不足，尚无实证基线 |
| P1.6 PAPER | 盘口模拟、成本明细、成交审计、预期 / 实际费用和净收益 | 功能 PASS；未取得真实成本外推证据 |
| P1.7 Position / Exit | 风险预算 / 仓位 / 杠杆、持仓复核、减仓、保护性退出、冷却 / deadband | 保守子集 PASS；独立新 Alpha 加仓未实现，默认禁止追加 |
| P2.8 CONFIRM | 模式展示与服务端硬锁 | **NOT IMPLEMENTED**：无私有订单适配、交易所仓位对账、真实撤单 |
| P2.9 AUTO | 模式展示与服务端硬锁 | **NOT IMPLEMENTED**：不能绕过锁定 |
| P3.10 Controlled Adaptation | 离线 Challenger、时间隔离 / 连续窗口报告、人工启动加载 | 部分完成；没有 Shadow / 调度 / 实盘晋升流程 |
| P3.11 AI Assist | OFF，整个产品核心不依赖 LLM | 可选项未启用 |

## 真钱上线门槛

| 合同 Gate | 当前结论 |
|---|---|
| A OBSERVE_READY | **尚未完成生产验收**：回放和结果闭环通过，需更长连续行情记录与网络稳定性证据 |
| B MODEL_HEALTH_READY | **尚未完成生产验收**：状态机与偏移测试通过，需足够真实可比样本及近期 / 历史校准基线 |
| C PAPER_READY | **仅功能就绪**：测试覆盖模拟经济账本；真实来源校准门槛仍会阻止未验证模型入场 |
| D CONFIRM_LIVE_READY | **FAIL / 未开放**：缺真实执行适配、小额资金测试、对账与执行证据 |
| E AUTO_LIVE_READY | **FAIL / 未开放**：D 未通过，也没有多窗口成本后正收益证据 |
| F RETRAIN_PROMOTION_READY | **FAIL / 未开放**：离线研究不等于 Shadow 与受控实盘晋升通过 |

## 运行与回滚

启动和测试命令见 [README](../README.md)。`./scripts/dev.sh` 启动公共真实行情，`--demo` 使用独立合成行情数据空间。真实行情不会因断网而被演示行情替换。

回滚模型：Ctrl+C 停止核心并保存 `data/`，通过 `--model` 显式加载保留的旧模型工件；重启模式恢复 OBSERVE。checkpoint 包含模型，移除 `--model` 参数仍会恢复已保存模型，不会自动回退为评分基线。如需重新观察默认未验证基线，应使用新的独立数据目录，原数据与账户记录保留；这会创建新的模拟账户，不是原账户内的模型回滚。更改原账户资金初值不能绕过已有风险记录。

运行环境、账户凭证要求和原始概率的解释见 [README](../README.md)；基线权重、概率计算、实时更新与离线验证边界见 [架构说明](ARCHITECTURE.md#标签与模型)。这些内容描述实现行为，不改变本文已执行的测试或真钱门槛结论。

构建工具链已下载到工作区 `.tools/`，Python 依赖在 `.venv/`，均被 Git 忽略。源码、锁文件、原始合同和截图可提交；数据、缓存、工具链和浏览器测试临时产物不会被提交。
