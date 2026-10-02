# Micro Alpha · 个人量化工作台

基于随附 `micro-alpha-trading-commercial-v1.0.1` 产品合同实现的本地个人版。面向 Binance / OKX 的 **BTC、ETH USDT 永续合约**，默认 **OBSERVE**。

当前交付为 **v0.1 观察 / 模拟版**：真实公共行情、订单簿、预测与自动结果、资格检查、模型健康、模拟成交、风险设置和可回放记录均已接通。**不提供真实下单通道**；CONFIRM / AUTO 在界面和服务端都不可启用。不能将代码测试通过等同于已证明盈利或已通过实盘门槛。

![工作台](docs/screenshots/workspace-desktop.png)

## 运行配置与账户

| 项目 | 当前个人版要求 / 建议 |
|---|---|
| CPU / 内存 | 建议 4 核及以上、8GB 起步，16GB 更适合同时开发和研究 |
| 磁盘 | SSD，初始预留约 10GB 用于依赖、构建与缓存；持续行情录制另需空间 |
| 操作系统 | 当前已在 macOS 验证；其他系统尚未验收，启动脚本需要 Bash 环境 |
| 软件 | Node.js 22+、Rust stable；离线研究当前验证版本为 Python 3.13 |
| 网络 | 能访问 Binance / OKX 公共 HTTPS、WSS，系统时间应同步 |
| GPU / AI 服务 | 不需要独立显卡，不需要 OpenAI 或其他 AI API Key |

硬件数值是当前 BTC、ETH 两个标的工作负载的部署估算，并非长时间容量压测结论。持续运行需要保持主机唤醒、核心进程运行；仅关闭网页不会停止后台采集，电脑休眠或核心退出则会中断采集。

**当前读取的公开行情不需要交易所账号、登录密码或 API Key。** PAPER 由本机模拟账本执行，没有连接交易所的模拟账户或真实账户。

| 操作 | 凭证要求与当前支持情况 |
|---|---|
| 当前接入的盘口、公开成交、标记价格等行情 | 不需要账户鉴权，已接入 |
| OBSERVE 预测、PAPER 本地模拟 | 不需要交易所凭证，已实现 |
| 读取个人真实余额、持仓、订单 | 需要账户 API 凭证，当前未实现 |
| 真实下单、撤单 | 需要交易权限的 API 凭证，当前未实现 |

未来若接入实盘，常规 HMAC 鉴权使用 Binance 的 API Key / Secret Key，或 OKX 的 API Key / Secret Key / API Passphrase；OKX 的 API Passphrase 是创建 API 时设置的口令，不是网页登录密码。当前程序没有凭证配置入口，也不需要提供网页登录密码。界面的“账户资金”“持仓”“净收益”均属于本地模拟账户。

接口鉴权依据：[Binance 安全类型](https://developers.binance.com/en/docs/products/derivatives-trading-usds-futures/general-info#endpoint-security-type)、[OKX 公开行情与 API 说明](https://www.okx.com/en-ae/help/okx-api-agreement)。

## 启动

需要 Node.js 22+ 和 Rust stable。此工作区已将 Rust 安装在 `.tools/`，启动脚本会自动使用；其他机器可安装 [Rust](https://rustup.rs)。

```bash
# 首次安装前端依赖
npm ci --prefix web

# 真实公共行情，不需要交易所 API key
./scripts/dev.sh

# 或使用完全独立的合成行情演示
./scripts/dev.sh --demo
```

打开 **http://127.0.0.1:3000**。核心监听 `127.0.0.1:8787`，前端通过同源 `/api` 代理访问。Ctrl+C 停止。两种行情源分别保存到 `data/live.sqlite` / `data/demo.sqlite`，不会混合模型统计。

更换独立数据目录：

```bash
MICRO_ALPHA_DATA=data/my-session ./scripts/dev.sh
```

需要直连交易所公共 HTTPS / WSS。WebSocket 不自动继承 HTTP 代理。连接或数据时间戳不符合门槛时，界面展示等待 / 过期，系统继续重连。只有演示启动参数会产生合成行情，断网不会自动降级成假行情。

## 如何使用

1. 在“市场工作台”选择 BTC / ETH，检查两家交易所的行情状态。延迟同时检查本机接收时间和交易所时间，不只检查 socket 是否连通。
2. OBSERVE 会自动创建、跟踪并关闭正式预测。核心判断固定到该次预测结束，实时价格与门槛每秒刷新。
3. “预测与交易”提供自动结果、MFE / MAE、误差分类、独立人工注释、历史加载与 JSON 导出。注释不改变真实标签。
4. “模型健康”展示可比样本、概率分桶、ECE、Brier、近期 / 历史基线及预测、EV、执行与市场状态偏移。
5. 设置个人风险预算与实际适用的成本假设。配置经过服务端验证，保存后生成版本与审计记录；已有持仓时禁止修改。
6. PAPER 仅在**新预测**通过全部资格门槛时模拟入场；不会因为切换模式追入已有信号。入场后持续重算资格与目标风险，可减仓、平仓；同一预测内不加仓。
7. 紧急停止立即阻止新模拟订单，默认保留持仓并继续 TP / SL / 超时保护。可在空仓时将行为改成 `flatten`。数据不可靠时，平仓保留为 `EXIT_PENDING`，不会使用旧价格假装成交。

初始真实市场模型是**未验证的确定性评分基线**，原始概率不冒充校准概率。其 Calibration 门槛不会通过，即使方向判断很强也不能模拟入场。演示模型有明确的 `synthetic-only` 标识，同样需要近期样本与可信度检查，不保证产生交易。

## 预测是否实时，以及如何理解概率

**实时描述行情和计算的更新方式；策略描述如何从行情形成判断。当前版本用实时行情执行文档第一阶段的确定性评分基线。**

| 环节 | 当前行为 |
|---|---|
| 行情接收 | WebSocket 事件到达后更新本地簿，并检查连续性和时间新鲜度 |
| 特征与交易资格 | 核心约每秒重算一次；网页在一次请求结束约 1 秒后再次读取状态 |
| 正式预测 | 每个标的同时一条；上一条结束、方向稳定至少 2 秒且距上次创建至少 5 秒后才建立 |
| 预测卡片 | 保存建立时的方向、原始 / 校准概率、期限与屏障；跟踪期间不覆盖原判断 |
| 结果判定 | 每个有效 Binance 簿事件检查目标 / 反向屏障，时钟事件也检查超时；自动保存结果 |

行情延迟、请求耗时和主机负载都会影响刷新时间，不能把“每秒计算”理解为端到端延迟保证。默认数据过期阈值为 3 秒；Binance 主行情失效会使在途预测 `INVALIDATED`，OKX 跨市场数据缺失也会阻止完整交易资格。是否继续形成观察预测取决于 Binance 主行情是否可用。

当前默认预测窗口为 60 秒，方向内目标为 0.30%，反向屏障为 0.10%，可在风险设置中调整。这些是初始化参数，不是经过研究选出的最优参数，也不是文档要求固定使用的唯一组合。

压缩包定义了特征、First-hit 标签、校准、风险和验收规则，以及 `确定性评分 → 逻辑回归 → GBT` 的模型推进顺序；**没有提供经过训练的权重或已证明有效的策略**。当前基线权重由实现阶段设定，逻辑回归和温度校准工具已经实现，但不会自动替换运行中的模型。

新建真实行情数据空间、未加载研究模型时，模型状态为：

```text
model_version: score-v1-unvalidated
model_type: deterministic-score
validated: false
calibration_version: none
calibrated_probabilities: null
```

此时“先涨 80%”表示评分经归一化得到的**原始模型输出**，不代表已经验证的 80% 命中率。基线原始输出没有显式使用预测期限、目标和反向屏障，不能直接当作该组条件下的可靠 First-hit 概率。未经校准时，界面显示“原始概率 · 尚未校准”，Calibration 门槛阻止生成可执行计划。

“核心判断”展示建立预测时的数值，“交易资格”使用最新行情重新计算，所以两处概率或 EV 可能不同；保存的初始预测仍用于最终误差评估。即使显示已校准，仍需通过近期可比样本、Model Trust、成本后 Net EV 和所有其他门槛。**有预测、校准通过、具备交易资格是三个不同状态。**

评分公式、特征用途和验证门槛见 [架构说明中的模型细节](docs/ARCHITECTURE.md#标签与模型)。默认值与实时状态分别以风险设置和 `/api/state` 为准；已有数据空间重启时会恢复其配置及模型，不能根据上述初始化值推断当前运行状态。

## 离线研究与 Parquet

```bash
python3 -m venv .venv
.venv/bin/pip install -r research/requirements.txt

# 将一份完整行情日志导出为 Zstd Parquet 分块
.venv/bin/python research/parquet.py data/live-<session>.jsonl
# 追加 --watch 可在引擎运行时持续导出；磁盘写入不进入行情计算路径

# 基线先做温度校准，再比较逻辑回归；使用真实来源的自动 Outcome
.venv/bin/python research/train.py \
  --db data/live.sqlite --source live --model score --output data/research-score
.venv/bin/python research/train.py \
  --db data/live.sqlite --source live --model logistic --output data/research-logistic

# 检查 report.json 后，明确选择离线模型；未通过研究门槛的模型会被核心拒绝
./scripts/dev.sh --model data/research-score/challenger.json
```

研究使用按时间顺序的训练 / 校准 / 验证分割，清除标签跨边界的样本，并比较三个连续验证子窗口。报告包含校准、成本后优势、单位名义价值回撤、尾部损失和强反向比例。模型工件记录特征与标签版本、窗口、成本假设、校准参数；加载时计算文件 SHA-256。真实数据不足时会拒绝报告或保持 `validated=false`。

此流程只支持人工选择 PAPER 研究模型；没有生产在线改权重、定时自动重训、Shadow 执行或自动晋升。模型窗口 / 屏障必须与配置匹配。更改配置后，Model Health 使用新的可比样本，重新积累可信度证据。

建议按下列顺序验证：

1. 使用真实行情 OBSERVE，保持研究窗口、屏障与成本配置一致，积累自动 Outcome；剔除数据失效，保留预测错误。
2. 用时间隔离的训练 / 校准 / 验证数据运行研究，检查 `report.json`，不能只看命中率或单一收益窗口。
3. 研究工具拒绝样本不足或门槛不通过的模型；通过时也只标记 `PAPER_RESEARCH_GATE_PASS`，不代表实盘许可。
4. 明确加载通过检查的模型，继续 OBSERVE，积累该模型和配置版本的近期可信度证据。
5. 满足全部门槛后使用 PAPER，比较预期与实际模拟成本、收益和退出行为；真钱阶段仍需另行实现与验收。

默认运行时最低可比样本数是每标的 30 条，这是可信度检查的起点，**不是训练完成或已经有效的证明**。当前离线工具另要求验证集至少 100 条、BTC / ETH 各至少 30 条，并检查校准、连续子窗口、成本后优势与尾部结果。具体规则见 [研究实现](research/train.py)。

如果启动时设置了 `MICRO_ALPHA_DATA`，研究命令的 `--db` 也应指向该目录。不能用演示数据库训练后，以真实来源模型名义加载。停止后重新启动同一数据空间会恢复已保存模型，移除 `--model` 不会自动回退；需要回退时请显式加载保留的旧模型，或使用新的独立数据目录观察未验证基线。

## 验证

```bash
./scripts/check.sh           # Rust 测试 / Clippy、前端构建、Python 研究测试
./scripts/replay-check.sh    # 240 秒固定录制，核对完整状态 SHA-256

# 核心与前端运行后，执行浏览器测试；测试会切换模拟模式和修改测试账户配置
cd web
npm run test:e2e
```

浏览器测试默认使用 macOS 已安装的 Chrome。其他平台可设置 `CHROME_PATH`，或运行 `npx playwright install chromium`。

直接回放任何完整录制：

```bash
source scripts/env.sh
cargo run --locked -- --replay data/live-<session>.jsonl
```

记录包含启动状态、规范化行情事件、每秒时钟事件和用户控制操作，回放不访问网络、不使用当前时间。JSON 浮点启用精确 round-trip，订单簿价格使用 1e-6 整数刻度。生产与回放调用同一核心。

## 实现与边界

- **Rust + Tokio**：行情适配、本地簿、特征、预测、First-hit、资格、风险、模拟执行、HTTP API。Node 不进入行情计算路径。
- **Next.js + TypeScript**：中文工作台、行情图、历史、健康、配置和操作审计。
- **SQLite WAL**：个人本地记录与恢复；后台专用线程处理持久化。资金、日亏损、历史峰值和持仓恢复后，自动回到 OBSERVE，并使中断的预测失效。
- **JSONL + Python / Parquet**：规范化录制、精确回放与离线研究。无需 PostgreSQL、Redis、Docker 或云存储。

Ground Truth 使用 **Binance 本地订单簿中间价**和规范化接收时间，按接收到的完整簿事件确定屏障先后；OKX 用于跨市场特征与资格检查。它不声称重建网络延迟期间未观测到的真实交易所路径。发生可检测的数据完整性问题会产生 `INVALIDATED`，不纳入学习。

模拟成交按可见盘口逐档撮合，支持部分成交、显式费用与滑点，假设 IOC 且模拟确认延迟为零。资金费是配置的保守成本预留，并非真实资金费结算；清算缓冲是风险估计，并非交易所清算价。超时的基线预期收益假设为零。成本参数需通过持续录制、PAPER 与未来小额实盘逐步验证。

尚未完成：真钱 REST / 私有 WS 订单生命周期、真实交易所最小下单量与持仓对账、撤单、保证金 / 清算规则、独立 Alpha 加仓、AUTO、可靠 OI / 清算流接入、Shadow 与受控实盘晋升。未提供虚假的可用开关或样例盈利证据。

完整阶段状态和验证记录见 [验收说明](docs/ACCEPTANCE.md)，原始文档保存在 [docs/spec](docs/spec/MASTER-CONTRACT.md)。
