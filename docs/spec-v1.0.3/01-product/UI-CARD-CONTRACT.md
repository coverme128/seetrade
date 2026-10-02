# UI Card Contract v1.0.3

## Primary Card

```text
BTCUSDT · OBSERVE

CORE RECOMMENDATION
LONG
Future 120s
+0.50% before -0.20%
P↑ 63% · P↓ 22% · Timeout 15%

WHY NOW
Order Flow       Strong ↑
Aggressive Buy   Rising ↑
Microprice       Bullish
Cross Venue      Confirmed
Liquidity        Good

TRADE QUALIFICATION
Calibration      PASS
Net EV           +0.08%
Cost             PASS
Slippage         PASS
Risk             PASS
STATUS           QUALIFIED

NEXT ACTION
Suggested Position   $8,000
Eff. Leverage        1.4x
Expected Net P&L     +$X
Planned Max Loss     -$Y
TP / SL / Timeout

MODE
OBSERVE | PAPER | CONFIRM | AUTO
```

## Live Tracking

显示：Start Price、Current Price、Current Move、Elapsed、Remaining、Target Distance、Stop Distance。

## De-leverage Display

必须显示当前与目标风险暴露：

- Current Position
- Target Position
- Current Effective Leverage
- Target Effective Leverage
- Current Action: HOLD / ADD / REDUCE / EXIT
- Action Reason

## Prohibited UX

禁止：

- 夸张 BUY NOW / 100x 文案
- 单一“AI Confidence”替代真实概率
- 允许用户一键把建议杠杆从低倍改成极高倍
- 只显示命中率不显示 Net EV / costs
- 把模型错误隐藏起来


## Episode Display Contract

- 一张 Card 对应一个 Prediction Episode，不对应一次 1s Evaluation。
- Card 可以每 1–2 秒刷新 live probability / Net EV / price progress，但不得因小幅数值波动新建 Card。
- Active 页面只保留仍有效 Episode；结束后自动进入 History/Review。
- 用户没有打开应用时，Episode 仍必须持续跟踪并自动结算 Outcome。
- OBSERVE 模式下仍显示系统本可给出的 Position / Effective Leverage / Expected Net P&L，但绝不产生真实订单。

## Result Truth

Result Card 必须把以下两类结果分开：

1. `Prediction Outcome`：TARGET_HIT / STOP_HIT / TIMEOUT / STRONG_OPPOSITE_MOVE / INVALIDATED。
2. `Trade Outcome`：仅在 PAPER/CONFIRM/AUTO 有执行时存在，包含 fees、slippage、fill、realized net P&L。


## AUTO Admission Display

当模式为 AUTO 时，Card 在 `Trade Qualification` 与执行动作之间必须显示一个简洁的 AUTO 结果：

- `AUTO: ADMITTED`：允许进入 Risk/Execution。
- `AUTO: SKIPPED`：机会仍可能是 QUALIFIED，但本次不占用真实资本。

若 SKIPPED，显示一个主要 reason code，例如：

- `AUTO_EDGE_THRESHOLD_NOT_MET`
- `MODEL_TRUST_INSUFFICIENT`
- `EXECUTION_QUALITY_FAIL`
- `RISK_BUDGET_UNAVAILABLE`
- `BETTER_OPPORTUNITY_ACTIVE`
- `REENTRY_COOLDOWN`

禁止把 `QUALIFIED` 直接渲染成“已自动下单”。
