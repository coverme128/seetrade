# Micro Alpha Trading — MASTER CONTRACT v1.0.1

## 1. Project Identity

**正式名称**：Micro Alpha Trading  
**正式定位**：Short-Horizon Microstructure Quant Trading System / 短周期市场微观结构量化交易系统  
**使用范围**：个人自用，非 SaaS，不做多租户、计费、组织权限、客户空间。  
**版本**：Commercial Personal v1.0.1

## 2. Core Product Goal

系统核心不是“自动交易机器人”，也不是“自主学习 AI”，而是：

> 实时识别短周期市场状态 → 形成核心交易建议 → 通过确定性计算、量化模型与可选 AI 辅助解释判断该建议是否具备足够条件概率和成本后正 EV → 通过 Trade Qualification Gate 后形成完整 Trade Plan → 用户可选择 OBSERVE / PAPER / CONFIRM / AUTO → 用真实 Outcome、Calibration、Model Trust、Drift 与 Realized Net P&L 持续验证系统是否仍然有效。

最终必须用：

`Expected Edge → Qualified Recommendation → Observed/Executed Outcome → Realized Net P&L → Model Health`

证明是否存在可持续 Alpha。

## 3. V1 Intelligence Boundary

v1.0.1 正式冻结为三层：

1. **Realtime Intelligence**：持续计算市场状态、概率、成本后 EV 与 Trade Qualification。
2. **Self Evaluation**：持续记录 Outcome，计算 Calibration、Model Trust、Drift、Expected vs Realized。
3. **Controlled Adaptation**：只有达到重训条件时才产生 Challenger；不得边交易边在线自修改生产模型。

V1 不以 Full Auto Learning / Online Learning / Auto Hyperparameter Search / Auto Promotion 为核心目标。

## 4. Non-Goals

本版本明确不做：

- SaaS / 多租户 / 团队协作 / RBAC
- 策略市场 / 社交 / 跟单
- 新闻爬虫与全网情报
- LLM 直接预测涨跌
- LLM 修改 Direction / Probability / EV / Position / Leverage / Exit
- 100x 杠杆卖点
- 大量小币种
- 一开始接 Bybit / Hyperliquid
- 一开始使用 Transformer / LSTM / RL
- 生产模型按单笔结果即时自修改
- 第一版 Full MLOps / AutoML / Online Learning
- 一开始做 Kafka/Flink/K8s 重架构

## 5. 1 + 4 Responsibility Model

### 01 Product
定义用户看到什么、系统如何表达建议、模式与核心体验。

### 02 Domain Rules
定义 Prediction、Label、EV、Trade Qualification、Position/Leverage、Exit、Model Trust、Drift、Learning 等不可被 UI 或实现绕过的交易规则。

### 03 Implementation
定义代码架构、数据流、接口合同、阶段实施顺序、测试与回放方式。

### 04 Production Handoff
定义 Codex 上线顺序、真实数据门禁、Paper/Live Gate、运维、Kill Switch、验收与回滚。

## 6. Priority Order

冲突时优先级：

1. MASTER CONTRACT
2. Domain Rules
3. Product
4. Implementation
5. Production Handoff 的操作细节

任何实现不得绕过 Domain Rules。

## 7. Frozen Items

- 产品核心 = Recommendation + Qualification + Optional Execution
- Prediction 采用 Triple Barrier / First-Hit 语义
- 第一阶段 Exchange = Binance + OKX
- 第一阶段 Asset = BTC + ETH
- Horizon 候选 = 5s / 15s / 30s / 60s / 120s
- Target 候选 = 0.05% / 0.10% / 0.15% / 0.20% / 0.30%；研究可扩展但不得把某个固定 Target 当产品前提
- 模式 = OBSERVE / PAPER / CONFIRM / AUTO
- AUTO 不是唯一模式；OBSERVE 是正式核心模式
- AI_MODE = OFF / ASSIST
- AI 不得直接改变 Direction、Probability、Net EV、Position、Leverage、Exit
- Risk Engine 权限高于 Alpha Engine
- 没有正的成本后 EV，不允许交易
- 杠杆只用于 Risk Scaling，不用于制造 Alpha
- Prediction Learning 与 Trading Learning 分离
- 每个正式 Prediction 必须自动形成 Ground Truth Outcome
- Calibration 与 Model Trust 是 Trade Qualification 的输入
- Drift 发生时先降风险/暂停，而不是自动修改生产模型
- 生产模型不得因单笔错误即时自修改
- Champion / Challenger / Shadow 可保留，但 V1 Promotion 默认受控，不要求自动升级

## 8. Mutable Items

可通过版本升级调整：

- Thresholds
- Horizon/Target/Stop 搜索空间
- Feature 子集
- Calibration 方法与刷新窗口
- Model Trust 公式
- Drift detection 窗口/阈值
- Cost Model 参数
- Position sizing 公式
- Rebalance Deadband
- UI 视觉样式
- 模型从 Deterministic → Logistic → GBT 的升级
- Retraining cadence

## 9. Codex Implementation Order

严格按以下顺序：

1. Market Data + Replay
2. Feature Engine
3. Triple Barrier Label + Outcome
4. Prediction + OBSERVE UI
5. Trade Qualification Gate
6. Outcome Tracking + Calibration + Model Trust + Drift
7. Cost Model + PAPER
8. Position / Leverage / Exit
9. CONFIRM + Small Live
10. AUTO
11. Controlled Retraining / Challenger / Shadow
12. Optional AI Assist

不得先做 AUTO 或 Full Auto Learning 再补 Prediction Validation。

## 10. Personal Live Gates

只有满足以下条件才可逐级进入真钱：

- Binance/OKX 市场数据连续性可验证
- 本地 Order Book 可检测 gap 并恢复
- OBSERVE Prediction 可自动形成 Outcome
- Calibration 有统计报告
- Model Trust 可根据近期真实表现降级
- Drift 可触发 Reduce Risk / Pause New Trade
- Cost Model 有 PAPER/Small Live 对比
- PAPER 与 LIVE 共用同一 TradePlan/Risk 逻辑
- Risk Gate 不能被 Alpha/AI/UI 绕过
- Kill Switch 可立即阻止新订单并执行配置的风险动作
- 每笔交易可还原 Recommendation → Qualification → TradePlan → Execution → Outcome
- 未证明成本后正 EV 前，AUTO 不得启用

## 11. Correction Rule

后续修正采用独立 correction bundle，必须包含：

- change reason
- impacted contracts
- migration steps
- rollback steps
- acceptance results
