# UI Card Contract

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
