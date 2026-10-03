# 个人运行与执行账户配置

本文对应当前个人版实现。硬件和软件准备见 [README](../README.md#运行配置与账户)，当前升级验收与证据边界见 [v1.0.4 验收](ACCEPTANCE.md)，原功能阶段见 [P0–P3 验收](ACCEPTANCE-P0-P3.md)。日常操作见 [运行时指南](RUNTIME-GUIDE.md) 和 [运维手册](OPERATIONS.md)。原始产品合同中的配置名称与本程序环境变量并非一一同名；以下以实际代码为准。

## 按使用方式准备

| 使用方式 | 必需配置 | 是否使用账户资金 |
|---|---|---|
| 合成行情演示 `--demo` | Node / Rust；不依赖交易所网络；`MICRO_ALPHA_EXECUTION` 留空 | 仅本地模拟账本 |
| 真实公共行情 OBSERVE | 两家交易所公共 HTTPS / WSS 可达、系统时间同步、可写的数据目录 | 否；不需要账号或 Key |
| 真实行情 PAPER | OBSERVE 的条件，加上匹配当前配置的已校准模型和足够健康样本 | 仅本地模拟账本；不需要 Key |
| 离线校准 / 定时研究 | Python 虚拟环境及研究依赖、当前配置的有效自动 Outcome | 否；不会自动晋升模型 |
| 交易所模拟账户 CONFIRM | 选择一家执行交易所；该交易所模拟环境的 API 凭证、账户模式及完整交易门槛 | 交易所模拟资金 |
| 真实账户 CONFIRM | 真实环境 API 凭证、`live` / `ALLOW_LIVE=YES`、账户及模型门槛 | 是；每次入场在界面确认 |
| AUTO | 对应执行环境配置、`ALLOW_AUTO=YES`、界面确认、独立 AUTO 样本与收益门槛 | 随执行环境决定 |

`--demo` 是**合成行情**；`MICRO_ALPHA_EXECUTION_ENV=testnet` 是**交易所模拟账户**；`PAPER` 是**本地模拟撮合**。三者不同。合成演示不要接入私有执行。公共行情始终同时观察 Binance / OKX，私有执行只选择其中一家；仅配置 OKX 账户并不取消 Binance 公共行情依赖。

## 启动配置文件如何生效

公共行情、OBSERVE、本地 PAPER 不需要密钥。私有执行适配器支持 Binance USDⓈ-M 和 OKX USDT SWAP，一个数据目录对应一个独立账户；同时只选一家执行交易所。交易信号仍同时观察两家公共行情。

首次从项目根目录复制 [`.env.example`](../.env.example) 为 `.env.local`，在本机填写；文件已被 Git 忽略。已有 `.env.local` 时直接编辑，避免覆盖。Binance 使用 Key / Secret；OKX 另需 API Passphrase。程序不读取网页登录密码，密钥不进入状态 API、数据库、录制或网页。默认不配置执行账户。

```bash
# 首次配置；已有文件则保留
test -e .env.local || cp .env.example .env.local
chmod 600 .env.local
```

Rust 核心、`scripts/start.sh` 和 `scripts/dev.sh` **不会自动读取 `.env.local`**，每个用于启动的新终端都要显式加载。字符串值使用 shell 单引号，尤其是含 `$` 等 shell 字符的口令。以下命令在项目根目录运行：

```bash
set -a
source .env.local
set +a
./scripts/start.sh
```

环境变量修改后需重新加载并重启核心。网页刷新不会使新凭证生效。当前验证数据位于 `data/live-validation`；模板使用 `data/personal`，不设置变量时使用 `data`。需要保留当前采集、配置、模型和样本时，将 `MICRO_ALPHA_DATA` 设为原目录；换目录会创建独立数据空间，不能据此认为历史已丢失。

同一时间运行一个核心实例；端口 `8787` / `3000` 已占用时先停止原实例。重启会回到 OBSERVE，且中断的预测按数据失效处理。

## 全部启动环境变量

| 变量 | 默认 / 允许值 | 何时填写 |
|---|---|---|
| `MICRO_ALPHA_DATA` | 未设置为 `data`；模板为 `data/personal` | 可写目录；SQLite、JSONL、后台研究结果均在该数据空间下 |
| `MICRO_ALPHA_EXECUTION` | 未设置或空：禁用；`binance` / `okx`：启用 | 仅私有账户接入需要 |
| `MICRO_ALPHA_EXECUTION_ENV` | `testnet` / `live`；默认 `testnet` | 私有执行环境；OKX 模拟交易也填 `testnet`，不是 `demo` |
| `MICRO_ALPHA_API_KEY` | 无默认值 | 私有账户的 API Key，须与环境匹配 |
| `MICRO_ALPHA_API_SECRET` | 无默认值 | 私有账户的 HMAC Secret；当前适配器不使用 RSA / Ed25519 私钥 |
| `MICRO_ALPHA_API_PASSPHRASE` | 默认空 | OKX 必填；Binance 留空；不是网页登录密码 |
| `MICRO_ALPHA_LIVE_CAP` | 默认 `100`，范围 `10–500` USDT | 私有执行名义敞口上限；模拟和真实环境均生效，不是充值额或保证金金额 |
| `MICRO_ALPHA_ALLOW_LIVE` | 仅精确 `YES` 放行；模板 `NO` | 使用真实资金端点的额外开关，不会直接切换交易模式 |
| `MICRO_ALPHA_ALLOW_AUTO` | 仅精确 `YES` 放行；模板 `NO` | AUTO 的额外开关，交易所模拟 / 真实环境都适用 |
| `MICRO_ALPHA_PYTHON` | 默认 `.venv/bin/python` | 后台手动 / 定时研究的解释器；可设为现有虚拟环境的绝对路径 |
| `MICRO_ALPHA_AI_MODE` | 模板 `OFF`；`OFF` / `ASSIST` | 明确关闭或启用本地解释；未设置时兼容旧版“配置模型名即启用”行为 |
| `MICRO_ALPHA_ASSIST_MODEL` | 无默认模型 | 仅 ASSIST 需要；指向本机已有的 Ollama 模型 |

只有启用了 `MICRO_ALPHA_EXECUTION` 才会读取并要求私有凭证。模板未填写凭证时保持观察即可。API 值仅在本机配置，不需要发到聊天、截图或提交到 Git。

## 交易所账户需要准备什么

| 项目 | Binance | OKX |
|---|---|---|
| 产品 | USDⓈ-M，BTCUSDT / ETHUSDT | USDT SWAP，BTC-USDT-SWAP / ETH-USDT-SWAP |
| 凭证 | HMAC API Key + Secret | API Key + Secret + API Passphrase |
| 模拟环境 | 对应 Futures 测试环境的凭证 | 在 Demo Trading 中创建的凭证 |
| 程序要求的持仓模式 | 单向，`dualSidePosition=false` | 单向净持仓，`posMode=net_mode` |
| 程序要求的保证金模式 | 单资产保证金，BTC / ETH 均全仓 | Futures mode，`acctLv=2`；订单使用全仓 |
| 杠杆校验 | BTC / ETH 的交易所杠杆均不超过 3 | BTC / ETH 的交易所全仓杠杆均不超过 3 |

私有查询需要账户读取权限，下单 / 撤单还需要对应合约交易权限；当前程序不调用提现接口，无需提现权限。只读 Key 可以用于账户查询，但不能完成下单验收。交易所权限与模拟凭证说明已按官方资料核对：[Binance 鉴权与测试环境](https://developers.binance.com/en/docs/products/derivatives-trading-usds-futures/general-info)、[OKX API 权限与模拟交易](https://app.okx.com/docs-v5/en/)。

这些账户模式是**本程序的兼容范围**，不是建议使用高杠杆。实际目标名义金额还受系统有效杠杆、风险预算和 `LIVE_CAP` 共同限制。账户须有对应环境的 USDT 可用保证金；首次接入使用无外部仓位 / 挂单的独立账户，程序不接管手工或其他程序的仓位。程序只核对账户模式，不替你切换设置。

在现有 `.env.local` 中选择下列一种配置；仅示例，不包含可用凭证：

```bash
# Binance 交易所模拟账户
MICRO_ALPHA_EXECUTION=binance
MICRO_ALPHA_EXECUTION_ENV=testnet
MICRO_ALPHA_API_KEY='在本机填写测试环境Key'
MICRO_ALPHA_API_SECRET='在本机填写测试环境Secret'
MICRO_ALPHA_API_PASSPHRASE=''
MICRO_ALPHA_LIVE_CAP=100
MICRO_ALPHA_ALLOW_LIVE=NO
MICRO_ALPHA_ALLOW_AUTO=NO
```

```bash
# OKX 交易所模拟账户；其余关闭项与上例相同
MICRO_ALPHA_EXECUTION=okx
MICRO_ALPHA_EXECUTION_ENV=testnet
MICRO_ALPHA_API_KEY='在本机填写模拟账户Key'
MICRO_ALPHA_API_SECRET='在本机填写模拟账户Secret'
MICRO_ALPHA_API_PASSPHRASE='在本机填写API口令'
```

真实环境还需换成真实环境凭证，并将 `MICRO_ALPHA_EXECUTION_ENV=live`、`MICRO_ALPHA_ALLOW_LIVE=YES`；不要为验证文档直接启用。当前验收尚未达到真钱运行条件。

## 界面内的策略、风险与研究设置

以下在风险设置中保存，版本化写入数据库，不是 `.env.local` 字段。已有数据目录恢复的是已保存值；下表只是源码初始化默认值，不是个人账户建议或收益结论。

| 配置 | 初始值 | 用途 |
|---|---|---|
| `equity` | 10,000 USDT | PAPER 初始权益；不是实盘账户余额，实盘权益来自交易所 |
| `risk_per_trade` | 0.0025，即 0.25% | 单次风险预算比例 |
| `max_daily_loss` / `max_drawdown` | 100 USDT / 5% | 日亏损 / 回撤限制；实际使用前按个人风险预算调整 |
| `max_position_notional` / `max_effective_leverage` | 5,000 USDT / 1 倍 | 系统目标名义金额与有效杠杆上限；私有执行还受更小的 `LIVE_CAP` 限制 |
| `horizon` / `target` / `stop` | 60 秒 / 0.30% / 0.10% | 预测期限和屏障，须与研究模型相符 |
| `fee_bps` / `slippage_bps` | 4 / 1 bps | 费用 / 滑点假设；1 bps = 0.01%，应按实际费率及成交证据校核 |
| `max_stale_ms` | 3,000 ms | 行情新鲜度门槛；不能靠放宽它掩盖积压 |
| `min_samples` / `high_trust_samples` | 30 / 100 | 当前可比样本检查；不等于离线训练已完成 |
| `kill_behavior` | `hold` | Kill 后阻止新风险并继续原保护；`flatten` 为排队减仓 |

研究窗口内保持配置一致；修改配置会形成新版本，新版本有效结果不足时研究不会借用旧版本通过门槛。不要通过降低校准、可信度或样本门槛强行产生交易。

研究依赖单独安装，前端 / Rust 启动不会自动安装 Python 包：

```bash
# 已有虚拟环境可直接使用，不必重建
python3 -m venv .venv
.venv/bin/python -m pip install -r research/requirements.txt
```

本机已验证 Python 3.13.3，依赖锁定在 `research/requirements.txt`。在研究面板手动运行或设置周期；定时研究默认关闭，允许 1 小时至 30 天，只生成候选和报告。样本不足时显示拒绝原因；通过离线报告后仍需影子验证和手动晋升。Parquet 导出是独立命令，需要时另行运行 `research/parquet.py --watch`，核心不会自动启动它。

可选 AI 保持 `MICRO_ALPHA_AI_MODE=OFF` 即可完整使用量化流程。ASSIST 需要另行启动本机 Ollama（`127.0.0.1:11434`）并指定已有模型；程序不自动下载模型，不需要 OpenAI Key，AI 也不参与下单。

## 接入、交易门禁与运行边界

先使用 `MICRO_ALPHA_EXECUTION=binance` 或 `okx`，保留 `MICRO_ALPHA_EXECUTION_ENV=testnet`。Binance 使用 `https://demo-fapi.binance.com`；OKX 使用模拟交易请求头，必须匹配模拟账户密钥。这与本地 PAPER 是不同的账户。公共信号来自真实公共行情，交易所模拟环境的价格和成交条件可能不同。

支持单向持仓、USDT 保证金、全仓、交易所杠杆不超过 3。OKX 要求 Futures mode（账户级别 2），不接受组合保证金。账户应专用于本程序。代码读取并核对模式，不主动改变交易所账户设置。名义金额另受 `MICRO_ALPHA_LIVE_CAP` 限制，默认 100 USDT，允许 10–500 USDT；最小数量和名义金额不够时拒单，不会向上取整突破风险上限。

填写凭证不等于开放下单：默认仍是 OBSERVE。新预测需通过校准、可信度、成本后 EV、两家行情和风险门槛，账户对账也必须通过，才能在 CONFIRM 中点击“执行当前计划”。预测和意图有有效期，发送前再次检查。真实资金端点另要求 `MICRO_ALPHA_EXECUTION_ENV=live` 和 `MICRO_ALPHA_ALLOW_LIVE=YES`。AUTO 另要求 `MICRO_ALPHA_ALLOW_AUTO=YES`、界面显式确认，以及至少 `max(high_trust_samples,90)` 笔同配置 PAPER 交易、三个连续窗口净收益为正、尾损 / 执行成本 / 当前模型健康检查通过。

订单处理采用 REST：意图落盘 → SUBMITTING 落盘 → 提交 IOC 限价 → 查询订单和成交 → 账户对账。ACK 不是成交；部分成交保留实际数量，超时进入 UNKNOWN 并按原 client ID 查询，不重发。明确拒绝与未知结果分开。交易费用、资金费和完成交易结果记录在 `broker_order`、`broker_snapshot`、`live_trade` 中。

风险退出使用 reduce-only 市价单；ADD 要求新形成的独立信号、增量 EV、间隔、次数、原止损风险预算，亏损不能触发加仓。CONFIRM 的入场逐笔确认，持仓后的减仓和保护退出自动执行。AUTO 允许按同一门槛自动入场和独立 ADD。全局 kill 阻止新增订单并撤销本程序未完成的入场；hold 保留原 TP/SL/期限保护，flatten 排队减仓平仓。不会取消其他程序的订单。

账户轮询每次完成后等待 5 秒，账户信息超过 10 秒阻断新增风险并触发已管理持仓的保护退出。账户查询、交易规则刷新、每个标的的开仓 / 加仓通道、减仓通道和撤单任务相互独立；订单意图每 100ms 检查，未完成订单约每秒查询，网络耗时另计。保护订单不等待账户查询或新的规则 HTTP 请求；首次缺少规则仍拒绝，缓存规则过期则只允许减仓。止损、止盈和期限保护由本机进程监控；当前未实现交易所原生条件止损或私有 WebSocket。电脑休眠、进程终止或交易所不可达时，无法保证及时退出。重启后模式回到 OBSERVE，先对账，未完成提交按 UNKNOWN 恢复。订单或账户不一致不会自动“修复”为已成交。

目前私有流程的验收证据来自本地模拟交易所和领域测试；没有使用真实 API Key，尚未完成交易所模拟账户或小额实盘验收。代码支持这些流程，不代表已获得真钱运行的有效性证据。交易所手续费和微观流动性应与本地成本配置核对，校准 / PAPER / Shadow 未通过时不会因为配置密钥而绕过限制。

接口依据：[Binance 订单与撤单](https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/New-Order)、[Binance 测试环境](https://developers.binance.com/docs/derivatives/usds-margined-futures/general-info)、[OKX 订单状态与对账](https://www.okx.com/docs-v5/trick_en/)。


## v1.0.3 AUTO 准入

普通交易资格与 AUTO 资本分配独立。当前 Qualified Episode 还必须通过 `auto_min_net_edge`、可信度、最新 Evaluation、执行质量、预算、并发数、冷却与安全次数上限。默认 AUTO 净优势 0.0004、要求 HIGH、最大并发 1、冷却 60 秒、300 秒最多 3 次入场、每日最多 20 次；可在风险设置中版本化修改，AUTO 优势门槛不得低于普通资格门槛。

同资产退出后须出现新的信号 epoch，增量净优势还须覆盖新增完整往返成本与 incremental_ev。每日 / 窗口统计保守计入本程序创建的所有实盘 ENTRY 意图（含 CONFIRM、后续取消或拒绝）；限制只阻止新入场，保护性退出不受次数上限阻止。它们绝不是交易次数目标。

竞争排序 `priority-v1`：成本后净优势降序 → 模型可信度降序 → 预期执行质量降序 → 风险占用升序 → Episode 开场时间升序 → Episode ID 字典序。执行质量定义为价差、预期滑点和市场冲击之和的负值。Hard fail 不参与预算分配。预算包含持仓及未完成订单预留，可用资金不足时后续候选被 SKIPPED；准入后仍受实际订单精度、最小金额、可用资金及共享风控约束。

每笔 AUTO ENTRY 意图含 `execution_mode=AUTO` 和独立 `admission`。发送前在核心中重算最新 Evaluation 和准入，缺记录、过期、换 Episode、模式变更、门槛失败或预算不足均拒绝。`auto_admission`、`live_risk_decision`、`live_risk_recheck` 与 broker 回报可串联审计；`/api/history?kind=auto_admission&limit=50` 或界面“加载准入审计”可查询。当前 Card 的 SKIPPED 表示当前不再分配新风险，已创建订单的历史 ADMITTED 仍保留在订单中。

OBSERVE / PAPER 下的 SHADOW 准入只是使用模拟账本的确定性预演，不发送订单，也不替代真实账户权限、多窗口策略验证和交易所模拟 / 小额实盘联调。


## 保护执行与实盘反馈补齐

平仓量以已确认成交账本为准，不以过期账户快照推断成交；因此部分成交的 ENTRY / ADD 尚待核实时，已确认部分也能发出 reduce-only 退出。保护原因锁定到本程序成交仓位归零且没有待处理订单；同时撤销仍可能新增风险的本程序订单，后续确认的新成交继续退出。同一标的只允许一个待处理减仓；明确拒绝后至少间隔 5 秒重试，UNKNOWN 只查询，绝不盲目重发。小于交易所最小数量的尾仓会保留待处理原因，不能假装平仓成功。发送前的 SUBMITTING 必须完成持久化，写盘后再核对 kill / 模式 / 保护状态。

每笔新意图冻结模型版本、配置版本、当时中间价、预计完整往返成本和净优势。已闭合真实交易聚合 ENTRY、独立 ADD、所有分批减仓的实际成交、手续费和资金费。实际执行成本 = 各腿相对意图参考中间价的不利成交差 + 手续费 − 资金费收入；预期成本只对 ENTRY / ADD 的实际成交量计入各自完整往返预算，净收益预期也按每次加仓当时的信号加权。它包含从意图到成交的价格变化，不声称是纯交易所滑点。

模型健康中的 `live` 单独报告最近 50 个同模型 / 同配置闭合交易的名义金额加权执行偏差与 EV 偏差。成本超出 `max_slippage` 即降为 DEGRADED；EV 需至少 `min_samples` 再与 `ev_drift_limit` 比较。结果只能降低现有可信度，不能抵消 LOW，也不能被良好的 PAPER 成绩抵消。缺少冻结参考的旧记录和版本不匹配的记录显示为排除样本，不补造历史。资金费后续对账会更新同一交易及指标，不生成第二笔交易。

执行面板补充成交账本量 / 账户量、快照年龄、当前 / 已批准目标名义金额及有效杠杆、止盈止损、动作、保护原因和待处理状态。目标表示已批准订单执行后的持仓（保护退出目标为零），不是尚未经过真实账户风控的模型建议；待核实不等于已成交。


资格阶段现直接读取所用账户的预算上下文；PAPER 日损益 / 回撤不会污染真实资格，真实挂单也会预留可用资金。同标的排队订单在重新核对自身总目标时不重复扣减。DEGRADED 会缩小最终目标名义金额，即便原目标受小额上限约束。

CONFIRM 与 ADD 的 SUBMITTING 边界也重新计算最新资格、方向、真实账户预算与目标仓位；新增风险不得超过当前目标。有效建议反向时，已有真实持仓进入保护性退出。AUTO ENTRY 保留独立准入及其发送前复核。


## v1.0.4 运行时触发配置

个人持续运行建议使用 `./scripts/start.sh`：先构建 Rust release 和 Next.js 生产版本，再启动；首次编译可能较慢。`./scripts/dev.sh` 保留给开发调试，启动前停止已有实例以释放 3000 / 8787 端口。原有环境变量和数据目录参数继续适用。

此升级无需新增环境变量、交易所 Key、数据库服务或 GPU。继续使用原行情与执行配置；公共行情与 OBSERVE / PAPER 仍不需要交易所凭证。核心和网页版本为 v0.4.0。

进入“模型健康 → 执行与研究”，展开运行触发诊断与配置，读取当前配置后编辑 JSON 并保存。只允许 OBSERVE、无持仓且无待处理订单。保存会推进触发配置和全局配置版本、关闭旧候选 / Episode、暂停 Shadow 并重新预热；配置保存在当前数据目录，重启保留。旧数据空间首次升级也推进配置版本，旧历史保留但不冒充新版本样本。

| 配置 | 默认与含义 |
|---|---|
| `normalization_method` | `robust_z_v1`；当前只接受此算法 |
| `sample_interval_ms` / `normalization_window_ms` | 250 / 120000；逐标的历史窗口最多 2,048 个样本 |
| `warmup_samples` | 32；历史不足不触发 |
| `abnormal_z` / `extreme_z` | 3 / 6；任意 EXTREME 或两个不同家族同窗 ABNORMAL 才唤醒 |
| `combination_window_ms` / `debounce_ms` | 1000 / 2000 |
| `candidate_ttl_ms` / `candidate_full_eval_interval_ms` | 10000 / 1500 |
| `active_full_eval_interval_ms` / `position_full_eval_interval_ms` | 均 1000；服务端固定验收范围 |
| `enabled_families` | 四个 `true`，依次为订单流、主动成交、盘口深度、价格速度 |
| `protection_spread` / `protection_depth` / `protection_slippage` / `protection_volatility` | 默认均开启；使用原风险门槛进行即时复核 |
| `depth_drop_fraction` | 0.5；相邻轻量采样的额外相对深度下降保护 |
| `protection_data_stale` / `protection_book_gap` | 必须开启，服务端拒绝关闭 |

完整范围由 `TriggerConfig::validate` 校验，未知字段与非法值会拒绝。推荐先保留默认值，用封存录制执行离线对照，再人工决定是否调整；系统不会根据短期表现自动修改阈值。固定步进只用于离线研究，不是常驻运行模式。验收和对照命令见 [当前验收记录](ACCEPTANCE.md)。


### 参数边界与配置所属位置

所有时间范围包含端点；z 与比例必须为有限数值。以下范围来自当前 `TriggerConfig::validate`，不是交易策略建议。

| 参数 | 服务端接受范围 |
|---|---|
| `normalization_method` | 仅 `robust_z_v1` |
| `sample_interval_ms` | 整数 100–1000 |
| `normalization_window_ms` | 整数 10000–600000；窗口 / 采样间隔的整数商 ≤ 2048 |
| `warmup_samples` | 整数 8–512，且样本数 × 采样间隔 ≤ 窗口 |
| `abnormal_z` | 1–20 |
| `extreme_z` | 大于 `abnormal_z`，且 ≤ 50 |
| `combination_window_ms` | 整数 100–2000 |
| `debounce_ms` | 整数 250–10000 |
| `candidate_ttl_ms` | 整数 3000–60000 |
| `candidate_full_eval_interval_ms` | 整数 1000–2000 |
| `active_full_eval_interval_ms` / `position_full_eval_interval_ms` | 必须为 1000 |
| `depth_drop_fraction` | 0.1–0.9 |
| `enabled_families` | 恰好四个布尔值 |
| 四个风险保护开关 | 布尔值；建议保留默认开启 |
| 两个数据保护开关 | 必须为 `true` |

窗口实际最多保留 2,048 个样本。未知 JSON 字段被拒绝；当前反序列化会对省略字段填入源码默认值，因此应从界面载入完整当前 JSON 后编辑，避免把省略字段误当成“保持原值”。保存相同配置也会推进版本。

| 配置层 | 设置位置 | 生效方式 |
|---|---|---|
| 数据目录、执行场所、凭证、AI / Python | 启动终端环境，或显式加载 `.env.local` | 重启核心；前端刷新不生效 |
| 风险、成本、屏障、AUTO 准入 | “风险设置” | 服务端校验后保存到当前数据空间，推进全局配置版本 |
| Pulse、候选节奏、触发及保护开关 | “模型健康 → 执行与研究 → 运行触发诊断与配置” | OBSERVE 空仓保存，推进触发及全局配置版本、重新预热 |
| 定时模型研究 | “模型健康 → 执行与研究” | 保存研究计划；只生成候选，不自动晋升 |

触发配置没有对应的 `MICRO_ALPHA_TRIGGER_*` 环境变量；不要把合同中的点分组名称直接添加到 `.env.local`。保留同一数据目录才能恢复其中的配置、模型、账本和历史。
