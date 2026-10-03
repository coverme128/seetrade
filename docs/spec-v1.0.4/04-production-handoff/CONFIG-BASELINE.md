# Config Baseline v1.0.4

The exact numeric thresholds are intentionally mutable and must be calibrated by replay. Do not treat this file as an alpha claim.

Required config groups:

```text
runtime.idle_mode = market_pulse
runtime.candidate_full_eval_interval_ms
runtime.active_full_eval_interval_ms = ~1000
runtime.position_full_eval_interval_ms = ~1000

trigger.normalization.method = percentile | robust_z
trigger.normalization.window
trigger.abnormal_threshold
trigger.extreme_threshold
trigger.debounce_ms
trigger.candidate_ttl_ms

trigger.order_flow.enabled = true
trigger.aggressor.enabled = true
trigger.book_depth.enabled = true
trigger.price_velocity.enabled = true

protection.spread.enabled = true
protection.depth.enabled = true
protection.slippage.enabled = true
protection.volatility.enabled = true
protection.data_stale.enabled = true
protection.book_gap.enabled = true
```

Production must not expose a user switch that re-enables all-assets fixed 1s Full Evaluation. Fixed-step mode remains research/replay only.
