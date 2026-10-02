# Implementation Architecture v1.0.3

## 1. Principle

个人自用，优先：低复杂度、可回放、热路径确定性、可审计。

## 2. Recommended Stack

### Trading Core
Rust + Tokio

模块：

- market
- book
- features
- alpha
- qualification
- cost
- risk
- position
- execution
- exchange/binance
- exchange/okx

第一版可部署为一个 `micro-alpha-engine` 进程，不拆微服务。

### Research
Python

- Polars
- NumPy
- scikit-learn
- LightGBM/XGBoost（升级阶段）

### Product Layer
TypeScript + Next.js

只负责：UI、设置、历史、控制，不进入交易热路径。

### Storage
- PostgreSQL: Prediction/Qualification/TradePlan/Trade/Execution/Outcome/ModelVersion/Config/Audit
- MinIO/S3 + Parquet: raw trades/book/features/replay/training datasets
- Redis: optional cache/state only

第一版不要求 ClickHouse、Kafka、Kubernetes。

## 3. Hot Path

`Exchange WS → Local Book → Feature → Alpha → Qualification → [Auto Admission when AUTO] → Risk/Position → Execution`

不得经：Node API / BullMQ / LLM / slow DB round-trip。

## 4. Replay First

所有核心算法必须能从录制的 MarketEvent replay，得到可复现实验结果。

Replay 是：

- Debug
- Backtest
- Regression test
- Model validation

的共同基础。


## 5. Cadence Separation

推荐默认节奏：

- Exchange WS / canonical book: event-driven
- Feature state: event-driven / approximately 100–500ms incremental
- Alpha + Trade Qualification evaluation: 1000ms default
- UI live metrics: 1000–2000ms
- Formal Prediction Episode: event/state-transition driven
- Model health aggregation: minute/hour rolling windows

核心原则：`calculate fast, publish selectively, act selectively, display stably`.

## 6. Episode Aggregator

`AlphaEvaluation` 进入 Episode Aggregator：

`Evaluation → hysteresis/debounce → create/update/close PredictionEpisode`

Episode Aggregator 必须位于 UI 之前，并由 Domain Rules 控制，不能只靠前端去重。


## 7. Auto Admission Location

`AutoTradeAdmission` 位于 Trade Qualification 之后、Risk/Execution 之前，仅在 AUTO 新开仓/重新入场路径启用。

它必须运行在 deterministic core 内，不调用 LLM，不通过 UI/Node/BullMQ 决策。Admission 可读取当前活跃候选集合以做透明的并发机会选择，但不得改变 Alpha Model 输出。
