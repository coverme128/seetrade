# Implementation Architecture

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

`Exchange WS → Local Book → Feature → Alpha → Qualification → Risk/Position → Execution`

不得经：Node API / BullMQ / LLM / slow DB round-trip。

## 4. Replay First

所有核心算法必须能从录制的 MarketEvent replay，得到可复现实验结果。

Replay 是：

- Debug
- Backtest
- Regression test
- Model validation

的共同基础。
