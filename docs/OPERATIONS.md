# 个人版运行、备份与故障排查

适用版本：v0.5.1 / 合同 v1.1（保留 v1.0.4 触发运行时）。动态币种迁移和冻结回退见 [动态币种池指南](UNIVERSE-GUIDE.md)。命令均从项目根目录运行；示例操作需按实际运行目录执行。本手册没有设置开机自启、后台守护或自动定期比较任务。

## 启动与停止

首次安装前端依赖：`npm ci --prefix web`。持续运行使用 `./scripts/start.sh`，它会先构建 Rust release 和 Next.js 生产版本，再启动两者；首次编译耗时较长。开发调试使用 `./scripts/dev.sh`。两个脚本都不自动加载 `.env.local`。

已有配置按 [加载步骤](EXECUTION.md#启动配置文件如何生效) 显式 source。仅观察公共行情且继续本工作区原历史，可使用：

```bash
MICRO_ALPHA_DATA=data/live-validation MICRO_ALPHA_EXECUTION='' MICRO_ALPHA_AI_MODE=OFF ./scripts/start.sh
```

首次个人使用可把数据目录改为 `data/personal`。独立合成演示使用：

```bash
MICRO_ALPHA_DATA=data/demo-guide MICRO_ALPHA_EXECUTION='' MICRO_ALPHA_AI_MODE=OFF ./scripts/start.sh --demo
```

以上为替代启动方式，同一时刻只启动一套实例。网页为 `http://127.0.0.1:3000`，核心为 `http://127.0.0.1:8787`。关闭网页不会停止核心；启动终端中的 Ctrl+C 会停止脚本启动的核心与前端。休眠、关机或退出核心会中断行情及本机持仓保护。需要有待处理仓位的运维时，先按 [账户运行边界](EXECUTION.md#接入交易门禁与运行边界) 处理，不能把停止进程当成平仓。

## 每次启动后的检查

1. 确认数据来源为 `live` 或预期的 `demo`，且使用原数据目录；重启应回到 OBSERVE。
2. 分别检查 BTC / ETH 和新增采集币种的各场所是否 ready，接收与交易所时间是否新鲜；单场所币种不满足原双场所交易资格。
3. 检查 `storage_healthy`、模型、各币种有效配置版本及扫描 / 动态池状态，等待 Pulse 重新预热；原历史保留，但在途预测因重启而失效。
4. 有执行账户时先核对账户、已确认持仓和待处理订单，再考虑切换模式；凭证本身不放行交易。

以下均为只读接口，可在服务运行时执行：

```bash
curl --fail --silent --show-error http://127.0.0.1:8787/api/health
curl --fail --silent --show-error http://127.0.0.1:8787/api/state
curl --fail --silent --show-error http://127.0.0.1:8787/api/trigger-analytics
curl --fail --silent --show-error 'http://127.0.0.1:8787/api/history?kind=protection_trigger&limit=50'
```

`/api/health` 的 `ok` 主要反映存储健康，不能替代各采集币种行情、模型健康和执行门槛检查。`/api/state` 的 `now` 为毫秒时间；与本机时间比较可观察核心积压。`telemetry` 提供 Pulse、完整评估、输入队列与 checkpoint 序列化的近期耗时，单位为微秒。

## 数据与日志放在哪里

| 路径（相对 `MICRO_ALPHA_DATA`） | 用途 |
|---|---|
| `live.sqlite` / `demo.sqlite` | 派生审计记录及最后 checkpoint，来源独立 |
| `*.sqlite-wal` / `*.sqlite-shm` | SQLite 运行时文件；进程运行时不要单独删除 |
| `live-<时间戳>.jsonl` / `demo-<时间戳>.jsonl` | 每次启动的新会话输入 Journal，供确定性回放与研究 |
| `research/` | 后台研究任务的工件与报告；具体目录以界面 job 信息为准 |

启动脚本默认将进程输出写到终端，不保证生成 `runtime.log`；已有验证目录中的该文件来自人工重定向。持续录制占用磁盘，当前没有自动归档、容量配额或历史清理任务。

SQLite `records.payload` 存的是完整包装对象 `{kind, payload, source, ts}`；具体事件内容在内部 `payload`。JSONL 保存输入 Journal，派生事件在回放时重建，不能用读取 SQLite 包装对象的方式直接解释 Journal。

## 停机备份与恢复

本节采用停机备份：先确认无需要继续处理的真实持仓 / 订单，正常停止实例并等待退出，再复制整个数据目录。不要在运行中只复制主 `.sqlite` 文件。用新目录避免覆盖旧备份：

```bash
set -e
runtime_data='data/live-validation'
backup_dir="data/backups/v0.5.1-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$backup_dir"
cp -Rp "$runtime_data" "$backup_dir/data"
git rev-parse HEAD > "$backup_dir/code-commit.txt"
git diff --binary > "$backup_dir/working-tree.patch"
```

同时保留交付源码、lockfile 和必要的未跟踪源码；`git diff` 不包含未跟踪文件。`.env.local` 另行安全保存，它没有包含在上述数据目录备份中。备份位置应有足够空间，且不要选在被复制的数据目录内部。

恢复时先停止当前实例，复制备份到一个不存在的新目录，不覆盖当前数据。示例中的备份时间戳应换成实际目录名：

```bash
set -e
backup_data='data/backups/v0.5.1-YYYYMMDD-HHMMSS/data'
restore_dir='data/restored-v0.5.1'
test -d "$backup_data"
test ! -e "$restore_dir"
cp -Rp "$backup_data" "$restore_dir"
MICRO_ALPHA_DATA="$restore_dir" MICRO_ALPHA_EXECUTION='' MICRO_ALPHA_AI_MODE=OFF ./scripts/start.sh
```

恢复会保留已存历史、配置和模型，重新建立行情簿并进入 OBSERVE。对于备份后发生过真实成交的账户，旧 checkpoint 不是最新成交事实，必须先核对交易所；不能直接恢复旧私有执行状态继续自动下单。

## 升级与回退

升级前停机备份数据和对应代码；在隔离目录完成回归和封存录制对照，再用原目录启动新版本。当前保存配置不会在每次启动时恢复成默认值。

- 首次从 v1.0.3 升级触发运行时会初始化触发配置版本并推进全局配置版本，旧结果保留，新样本按当前配置积累。
- v1.1 动态池采用追加迁移，原 BTC/ETH 历史保留，扫描和组合实盘默认关闭。
- 0.5.0 → 0.5.1 兼容旧档案缺少独立版本字段的检查点；缺失时沿用全局版本，不因重启自动分配单币新版本。已配置 `venue_costs` 的资产需主动重新保存并重新研究，见 [升级检查](UNIVERSE-GUIDE.md#升级到-051-后的检查)。

2026-10-04 的 0.5.1 升级备份为 `data/backups/v0.5.0-before-v0.5.1.sqlite`，包含 51,280 条记录；升级后旧记录缺失或变化为 0，完整性及外键检查通过，见 [本机升级证据](evidence-v1.1/fix-upgrade.json)。该次 SQLite 备份通过 backup API 创建；日常操作仍按上面的停机整目录流程，同时保存 JSONL、研究工件及代码。

先在 OBSERVE / PAPER 检查数据与运行行为，再依据账户门槛决定是否使用 CONFIRM / AUTO。v1.0.4 历史 G7 是本机 5 分钟 OBSERVE 证据，v1.1 另有三币短时行情检查；这些均不代替多币长期、12 槽位持续负载或私有账户验收。

需要暂停动态池时，优先使用“冻结动态池并切回观察”，让新增币种已有仓位 / 待处理订单继续保留行情与保护，历史和档案不删除。解除冻结不会自行打开扫描或实盘。

程序版本回退必须先核对真实账户，处理仍需保护的仓位和待处理订单，再停止实例、封存本次录制与审计，使用已知可用的旧代码及其对应备份。旧版不能直接接管动态币种持仓，不能通过旧库撤销实际成交。固定步进仅作离线参考；当前回退流程见 [动态池回退](UNIVERSE-GUIDE.md#实盘与回退)，旧版本迁移记录保留在 [v1.0.4 历史验收](ACCEPTANCE.md#迁移回退与证据边界)。

## 封存录制与回放

使用正常停止后关闭的完整会话 JSONL，或此前已经封存的会话；不要把仍在追加的当前日志当成完整比较证据，也不要拼接多次会话。下例使用仓库夹具验证命令：

```bash
source scripts/env.sh
mkdir -p data/diagnostics
cargo run --locked --example trigger_fixture -- data/diagnostics/trigger-fixture.jsonl
cargo run --locked --example trigger_compare -- data/diagnostics/trigger-fixture.jsonl > data/diagnostics/trigger-comparison.json
cargo build --locked
target/debug/micro-alpha-engine --replay data/diagnostics/trigger-fixture.jsonl
```

对自己的封存录制，把命令中的输入路径替换为实际 JSONL。`--replay` 重放输入 Journal；`trigger_compare` 则从冻结模型和配置重算事件驱动 / 固定步进两路研究结果，忽略并计数非配置控制记录，不能用其核对真实订单账本。

`./scripts/replay-check.sh` 另验证两套冻结摘要；`./scripts/check.sh` 执行项目回归。浏览器测试会修改模式和配置，应在独立数据目录的 `--demo` 实例上运行。已完成预测卡测试使用固定合成结果，冻结行情测试控制浏览器时钟，不再依赖新实例等待闭合结果。不要对正在采集的个人真实数据实例运行完整浏览器测试。

当前没有周期性回放比较调度器。建议积累新封存录制、市场状态变化或准备修改阈值时手动比较，并保留配置与报告；后台“定时研究”生成模型候选，不会代替触发对照或自动修改生产阈值。

## 常见现象与处理

| 现象 | 检查与处理 |
|---|---|
| 3000 / 8787 被占用 | 用 `lsof -nP -iTCP:3000 -iTCP:8787 -sTCP:LISTEN` 识别占用进程，停止自己启动的旧实例；不对未知进程盲目终止 |
| 核心已连接，但行情未就绪 | 检查两家公共 HTTPS / WSS、系统时间、盘口断档和重连原因；WS 不自动继承 HTTP 代理 |
| IDLE 且完整评估次数不增长 | 健康预热完成且未达到组合触发条件时属正常；看 Pulse、触发判断和四路行情 |
| 有触发但没有新预测 | 检查候选 TTL、方向确认、FLAT 和已消费的 signal epoch；触发不直接生成 Episode |
| 有预测但不交易 | 查看校准、Model Trust、成本和风险原因；AUTO 还需要独立准入及执行证据 |
| 出现 DATA 保护 | 查 `protection_trigger` 原因：时间偏差、过期、断连或坏簿；恢复健康后重新预热，不能通过放宽过期门槛掩盖积压 |
| CPU 高、核心时间落后 | 确认使用 release 启动；检查输入队列、checkpoint 耗时、并行编译和磁盘负载；保留问题时段诊断 |
| 存储不健康或磁盘不足 | 先处理执行状态，再停机备份 / 归档已封存录制、恢复可写空间；不要删活动 WAL 或把写入失败当作成功 |
| 升级后当前可比样本下降 | 核对该资产有效配置版本及模型版本，变化后按新上下文积累；旧历史仍存在，不借旧版本样本绕过门槛 |
| 修改 ETH 后担心 BTC 样本失效 | 0.5.1 使用独立有效配置版本；检查是否同时保存了全局配置或全局触发配置，后两者会改变全部资产上下文 |
| 升级前配置过 `venue_costs` | 重新保存该币种完整配置，再积累新样本并研究；旧成本记录不改写，旧工件不作为新口径晋升证据 |
| 扫描 HIGH，但雷达没有好机会 | HIGH 表示研究优先级；还需采集、模型与触发校准、正式 Episode 和交易资格，雷达默认过滤无效机会 |
| `book_stability` 硬失败 | 查看最近一小时重置 / 重连次数；超过 3 次触发硬失败，不能用进程累计次数直接判断；排查连接与簿连续性后重新积累健康数据 |
| 共享模型导入提示独立配置 | BTC/ETH 已使用独立配置时，按币种运行研究与 Shadow，不用共享工件覆盖独立档案 |
| 漏斗全为零、对照没有漏机会 | 检查报告范围、预热、未闭合结果和有效参考分母；零分母不构成零漏机会证据 |

反馈问题时保留程序版本、数据来源 / 目录、发生时间、配置版本、相关 Episode / Trigger ID 和脱敏后的诊断。账户记录可能包含个人交易信息；不要附带 `.env.local` 或 API 凭证。
