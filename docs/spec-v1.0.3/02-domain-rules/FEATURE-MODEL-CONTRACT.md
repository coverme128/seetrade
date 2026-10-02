# Feature & Model Contract

## 1. First-Phase Features

只优先验证：

1. Order Book Imbalance
2. Order Flow Imbalance
3. Aggressive Trade Imbalance
4. Microprice - Mid
5. Short-term Momentum
6. Spread
7. Depth / Liquidity
8. Short-term Realized Volatility
9. Cross-Venue Lead/Lag
10. Liquidation / forced-flow（可靠时）

Funding / OI 主要作为 regime/context。

## 2. Model Order

必须遵循：

Deterministic Score → Logistic Regression → Gradient Boosted Trees

禁止第一阶段直接：LSTM / Transformer / RL / LLM price prediction。

## 3. Calibration

用户与 Position Engine 使用 calibrated probability，而不是 raw score。

必须跟踪：

- Brier / log loss（可选其一或两者）
- reliability bins
- predicted vs actual hit rate
- regime-specific calibration

## 4. Model Stability

任何生产模型必须记录：

- training window
- validation window
- feature schema version
- label definition version
- cost assumptions
- model artifact hash
