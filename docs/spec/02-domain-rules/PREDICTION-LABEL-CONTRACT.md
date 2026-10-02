# Prediction & Label Contract

## 1. Core Label

优先使用 Triple Barrier / First-Hit。

给定：

- P0
- Upper Barrier +X%
- Lower Barrier -Y%
- Timeout T

Long：

- Upper first → LONG_WIN
- Lower first → LONG_LOSS
- neither before T → TIMEOUT

Short 可对称或独立建模。

## 2. Research Grid

基线 Horizon：5s / 15s / 30s / 60s / 120s

基线 Target：0.05% / 0.10% / 0.15% / 0.20% / 0.30%

Stop 不固定，允许由 volatility / MAE / EV 搜索决定。

## 3. Prediction Output

必须输出：

- symbol
- timestamp
- horizon
- upper_barrier
- lower_barrier
- p_up_first
- p_down_first
- p_timeout
- calibrated probabilities
- expected_move
- expected_edge_before_cost
- expected_edge_after_cost
- signal_strength
- model_version
- feature_snapshot_id

## 4. Outcome

系统使用未来真实行情自动形成 Ground Truth，不依赖用户标记“对/错”。

Outcome 至少：

- TARGET_HIT
- STOP_HIT
- TIMEOUT_POSITIVE
- TIMEOUT_NEGATIVE
- STRONG_OPPOSITE_MOVE

附带：MFE、MAE、final move、first-hit timestamp。
