# Learning & Model Health Contract v1.0.3

## 1. Purpose

V1 的目标不是构建“会不断自我改写的交易 AI”，而是让系统：

1. 会持续计算；
2. 会自动知道预测结果；
3. 会判断自己最近是否仍可靠；
4. 发现失效时先降低风险；
5. 必要时受控重训。

## 2. Two Independent Feedback Loops

### 2.1 Prediction Evaluation

`Prediction → Market Outcome → Calibration / Error / Model Trust`

回答：预测是否仍然可靠。

### 2.2 Trading Evaluation

`TradePlan → Fill → Actual Cost → Realized Net P&L → Execution Quality`

回答：即使预测存在 Edge，真实成交后是否还能赚钱。

二者必须分开保存、分开评估。

## 3. Ground Truth Is Automatic

每个正式 Prediction 必须由后续市场数据自动关闭，不依赖用户手动“对/错”标记。

Outcome 至少包括：

- TARGET_HIT
- STOP_HIT
- TIMEOUT_POSITIVE
- TIMEOUT_NEGATIVE
- STRONG_OPPOSITE_MOVE
- INVALIDATED（仅限数据完整性/交易所异常等非市场结果）

必须记录：

- prediction timestamp
- market/feature snapshot
- horizon / target / opposite barrier
- raw probability
- calibrated probability
- qualification result
- model version
- actual first-hit result
- MFE / MAE
- final move at timeout
- prediction error class

## 4. Human Annotation

用户可以增加上下文标签：

- unexpected volatility
- market-wide reversal
- liquidation cascade
- exchange anomaly
- suspected news/event
- other

Human Annotation 不得覆盖自动 Ground Truth，也不得直接进入生产模型权重更新。

## 5. Calibration

系统必须持续比较：

`Predicted Probability Bucket vs Actual First-Hit Frequency`

例如：模型长期输出 60–65%，实际若只有约 49%，必须标记 Calibration Deterioration。

V1 必须支持：

- probability bucket statistics
- calibration error
- recent vs baseline comparison
- raw probability 与 calibrated probability 分离

交易决策优先使用 calibrated probability。

## 6. Model Trust

Model Trust 与单次 Confidence 分离。

建议状态：

- HIGH
- NORMAL
- DEGRADED
- LOW

Model Trust 至少考虑：

- recent calibration error
- recent observed Net EV vs expected Net EV
- wrong-way / strong-opposite rate
- recent MAE distribution
- execution drift（若进入 PAPER/LIVE）
- regime/OOD state
- sufficient recent sample check

Trade Qualification 必须读取 Model Trust。

默认行为：

- HIGH/NORMAL：按正常 Gate 评估
- DEGRADED：降低 Position Multiplier 或提高 Qualification 门槛
- LOW：禁止新 LIVE/AUTO 交易；OBSERVE 可继续

阈值不得在产品合同里拍脑袋写死，由 research config 决定。

## 7. Drift Monitoring

至少监控四类 Drift：

### Prediction Drift
Predicted hit rate vs actual hit rate。

### EV Drift
Expected Net EV vs observed/paper/live Net EV。

### Execution Drift
Expected spread/slippage/fees vs actual execution cost。

### Market Regime Drift
Volatility、spread、depth/liquidity、cross-venue relationship 与训练/验证分布的偏离。

## 8. Drift Response Order

检测到 Drift 时，系统必须按以下顺序响应：

`Trust Down → Position Down / Qualification Harder → Pause New Trades if Severe → Retrain Recommended`

禁止：

`Drift detected → production model immediately rewrites itself`。

## 9. Retraining Trigger

V1 允许自动提出 `RETRAIN_RECOMMENDED`，但不要求自动替换 Champion。

触发条件可包括：

- calibration error 超过研究阈值
- observed Net EV 显著低于基线
- Model Trust 持续 DEGRADED/LOW
- enough new observations accumulated
- material regime shift

## 10. Controlled Retraining

推荐流程：

`Learning Dataset → Train Challenger → Walk-forward/OOS → Shadow → Compare → Human/Controlled Promotion`

Promotion 必须比较：

- OOS calibration
- cost-adjusted Net EV
- drawdown
- tail loss
- strong-opposite rate
- stability across windows/regimes
- execution compatibility（若适用）

## 11. No Instant Self-Mutation

严格禁止：

`单笔预测错误 → 修改生产模型 → 下一笔真钱立即使用新参数`

V1 不实现：

- online weight mutation
- self-directed AutoML in production
- automatic hyperparameter search in hot path
- unchecked automatic Champion promotion

## 12. Minimum V1 Learning Capability

P0/P1 必须有：

- Outcome Tracking
- Calibration
- Model Trust
- Drift Monitoring
- Retrain Recommendation

可后置：

- Scheduled Retraining
- Challenger
- Shadow
- Promotion

不要求：Full Auto Learning。


## 13. Episode-Level Learning Unit

Calibration、Model Trust 与 Drift 的最小统计单位是已经关闭的 Prediction Episode，而不是每秒 Evaluation。

禁止把同一个连续机会的 60 次一秒 Evaluation 当作 60 个独立预测样本，否则会夸大样本量并污染 Calibration。

## 14. No-Trade Still Learns

OBSERVE / skipped CONFIRM 的 Episode 仍必须进入 Prediction evaluation dataset；只有 Trading evaluation dataset 需要真实或 paper execution。
