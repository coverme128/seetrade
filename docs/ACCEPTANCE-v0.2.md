# 交付与验收记录 · v0.2 · 2026-10-02

以原合同 P0 → P1 → P2 → P3 为顺序补齐功能实现。原需求文件 `docs/spec` 未修改。本表区分代码与自动化验证、真实市场有效性、私有账户联调；不得互相替代。默认 OBSERVE，真实资金与 AUTO 均关闭。

## 阶段对应

| 原始优先级 / 阶段 | 已接入的产品能力 | 验收边界 |
|---|---|---|
| P0 / 1 Market + Replay | Binance / OKX BTC、ETH 深度、成交、mark/index、资金费上下文；序列恢复、SQLite / JSONL / Parquet、确定性回放 | 公共行情短测通过；长期稳定性仍需积累；OI / 强平流按“可靠时”条件未启用 |
| P0 / 2 Features + Outcomes | 首批微观特征、跨市场收益差、First-hit、MFE / MAE、失效样本排除；录制质量报告 | 断档、陈旧、迟到、截尾和不可学习结果可识别；跨市场 lead/lag 是 1 秒收益差代理 |
| P0 / 3 Prediction | score / logistic、按 regime 校准、生命周期、原卡结果、独立信号强度与校准后的预期方向幅度 | 默认真实模型未校准；expected_move 是 First-hit 收益近似，超时预期为零 |
| P0 / 4 Qualification | 概率、校准、健康、EV、成本、流动性、波动、OOD、风险与数据门槛 | LOW / 缺校准 / 负 EV 均不能生成可执行计划 |
| P1 / 5 Model Health | 近期与基线、分桶、regime 证据、Prediction / EV / Execution Drift、重训建议 | 降级不会修改冻结权重 |
| P1 / 6 Cost + PAPER | 簿内 IOC / 部分成交、费用 / 滑点、预期与实际成本；按公布费率与时点模拟资金费 | PAPER 成交即时确认，非网络延迟模拟；公共资金费记账是估计，实际扣款由私有账单对账 |
| P1 / 7 Position | 独立信号重新形成后的 ADD、HOLD / REDUCE / EXIT；间隔、次数、deadband、增量 EV 与原止损预算 | 不因亏损加仓，不延长原持仓期限或扩大原止损 |
| P2 / 8 CONFIRM | Binance / OKX HMAC REST、账户模式 / 精度 / 最小量、Execute / Skip、独立小额上限、撤单、部分成交、费用 / 资金费 / 交易损益对账、kill、持久化与恢复 | 已做本地模拟接口测试，尚无交易所模拟账户 / 小额真实账户联调证据 |
| P2 / 9 AUTO | 环境与界面双重显式启用；当前模型、同配置 PAPER 多窗口证据；共享资格 / 风险，自动入场、独立 ADD、减仓和退出 | 默认关闭；不存在绕过校准、账户对账或风险门槛的 AUTO 开关 |
| P3 / 10 Adaptation | 手动 / 定时任务、GBT 浅树候选、walk-forward、Shadow、显式晋升 / 回退；期限 / 屏障研究搜索 | 任务不修改 Champion；未证明真实策略达到研究或晋升门槛 |
| P3 / 11 Optional Assist | 默认事实摘要，可选本机 Ollama 解释信号、漂移、结果与研究报告 | 完全只读、脱离交易主循环；本机没有模型时使用事实摘要 |

## 已执行检查

- Rust：51 项测试，覆盖领域、执行状态、Binance / OKX 本地模拟接口、恢复与 Shadow。
- Python：11 项测试，包含 GBT 全流程报告、时间隔离、数据质量、Parquet。
- 浏览器：10 项测试，包含桌面 / 手机、设置恢复、Shadow、研究任务、只读解释和内部回报接口防伪。
- Rustfmt / Clippy（警告视为错误）、TypeScript、Next.js 生产构建通过。
- 240 秒录制：3,122 条记录、11 个 Outcome、0 笔交易；在线计算与离线回放完整状态 SHA 一致。新增序列化字段后更新基准，摘要见 `fixtures/demo-240.sha256`：`352973c9f253c9e49798ac06fbabcf19d0f915e8cb60adb1cff5d67dc1c05652`。
- 新版本恢复旧数据目录、默认 OBSERVE；Binance / OKX 两标的 mark/index 与盘口短测见 `priority-smoke.json`。没有配置交易凭证，没有发真钱订单。
- 实际提交一次 score 研究任务，数据不足 100 个不同时间点时明确拒绝，状态 FAILED、原因可见；Champion 未改变。
- 3,000 秒合成录制的参数搜索实际运行：75 组训练候选，后段 44 个样本、93 条尾部截尾路径；录制质量报告 PASS。结果仅验证研究流程，不代表市场表现。

## 执行的不确定性处理

执行意图与 SUBMITTING 先确认落盘，再发请求。明确交易所拒绝记为 REJECTED；超时 / 模糊错误记为 UNKNOWN，查询同一个 client ID，不自动重发。账户持仓必须与所有已确认成交量相符；外部持仓、外部挂单、资金不足、超限、成本异常或陈旧快照均阻断新增风险。浏览器不能提交 `broker_*` 或伪造研究完成事件。

真实订单独立记录在 `live.orders`，完成交易在 `live.trades` / `live_trade`；不会写入本地 PAPER 账本。迟到的资金费可产生独立调整记录。每个意图包含原 Prediction / Qualification / Plan，风险决策附账户快照和额度。

私有账户配置、支持的保证金模式及运行条件见 [EXECUTION.md](EXECUTION.md)。实现采用 REST 轮询和本地止损监控，尚无私有 WS 或交易所原生条件保护；本机退出时不能保证继续执行退出规则。这一边界须保留在私有账户验收记录中。

## 迁移与回退

新增可持久化字段提供 serde 默认值；旧 SQLite checkpoint 可读取，重启使行情失效并回到 OBSERVE，执行通道必须重新对账。运行时性能计时不参与回放摘要。模型新增加可选固定树结构；旧评分 / logistic 工件仍按原 schema 读取。模型记录固定配置成本假设，与当前周期是否跨资金费结算分开，避免动态资金费破坏工件匹配。

升级前保留数据目录副本。回退旧程序时使用升级前的数据目录；旧程序无法解释新执行命令。不要在有 UNKNOWN、未完成订单或真实持仓时切换账户 / 执行环境。模型本身仍通过现有手动回退流程处理，不会清零账户损益。

历史验收保存在 [ACCEPTANCE-v0.1.md](ACCEPTANCE-v0.1.md)。所有盈利、持续稳定性与真实执行环境的门槛仍须以对应真实证据验收，代码测试只证明所覆盖的行为。
