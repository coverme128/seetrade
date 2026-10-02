# Prediction & Label Contract v1.0.3

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


## 5. Evaluation vs Prediction Episode

### Evaluation

- 默认 cadence：约 `1000ms`。
- 是内存中的实时重算结果，不要求逐条持久化。
- 可更新 Probability、Net EV、Drivers、Qualification 和 Target Position。
- Evaluation 本身不是一条用户可见正式预测。

### Prediction Episode

只有满足 Episode creation / transition rule 时才创建或更新正式记录。至少包含：

- episode_id
- symbol
- opened_at / closed_at
- direction
- horizon
- upper_barrier / lower_barrier
- opening probability snapshot
- latest probability snapshot
- opening feature_snapshot_id
- latest feature_snapshot_id
- highest qualification reached
- episode status
- close reason

同一连续机会不得因为每秒 Evaluation 产生重复 Episode。

## 6. Episode Closure

所有正式 Episode 必须自动关闭，关闭条件至少包括：

- TARGET_HIT
- STOP_HIT
- TIMEOUT
- SIGNAL_INVALIDATED
- DATA_INVALIDATED（仅数据完整性或交易所健康异常）

`DATA_INVALIDATED` 不得伪装成预测正确或错误。

## 7. Prediction Truth

系统必须保留用于事后复核的客观事实：

- Start Price
- First Hit
- First Hit Timestamp
- MFE / MAE
- Final Move at Timeout
- Raw / Calibrated Probability at Episode Open
- Model Version
- Feature Schema Version

Ground Truth 来自真实市场路径，不依赖 AI 或用户主观标记。
