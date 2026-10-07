"use client";
import { useCallback, useEffect, useRef, useState } from "react";

type Counts = Record<string, number>;
type Summary = {
  opportunities: number;
  qualified: number;
  pending: number;
  eligible: number;
  invalid: number;
  paper_entries: number;
  exchange_filled_opportunities: number;
  observed_net_edge: number | null;
  target_hit_rate: number | null;
  calibration_samples: number;
  calibration_ece: number | null;
  hit_probability_brier: number | null;
  outcomes: Counts;
  invalid_reasons: Counts;
  execution: Counts;
  non_execution_reasons: Counts;
};
type Trade = {
  id: string;
  source: string;
  channel: string;
  venue: string;
  prediction_id: string;
  symbol: string;
  model_version: string;
  config_version: number | null;
  closed_at: number;
  exit_reason: string;
  net_pnl: number | null;
};
type Row = {
  prediction_id: string;
  source: string;
  symbol: string;
  opened_at: number;
  direction: string;
  model_version: string;
  config_version: number;
  opening_grade: string;
  ever_qualified: boolean;
  outcome: string | null;
  learning_eligible: boolean;
  invalid_reason: string | null;
  observed_net_edge: number | null;
  execution: string;
  reasons: string[];
  horizon_observation: {
    status: string;
    final_move_at_timeout: number | null;
  } | null;
  closed_trades: Trade[];
  orders: {
    id: string;
    channel: string;
    venue: string;
    status: string;
    filled_qty: number;
  }[];
  shadow_admission: { decision: string; reasons: string[] } | null;
};
export type ReviewReport = {
  schema: string;
  date: string;
  timezone: string;
  as_of: number;
  through_id: number;
  provisional: boolean;
  latest_record_ts: number | null;
  summary: Summary;
  cohorts: {
    source: string;
    symbol: string;
    model_version: string;
    config_version: number;
    metrics: Summary;
  }[];
  trading: {
    source: string;
    channel: string;
    venue: string;
    metrics: {
      closed_trades: number;
      net_pnl: number | null;
      win_rate: number | null;
      average_net_pnl: number | null;
      realized_pnl_drawdown: number | null;
      mean_cost_overrun: number | null;
      cost_samples: number;
    };
  }[];
  trade_cohorts: (ReviewReport["trading"][number] & {
    symbol: string;
    model_version: string;
    config_version: number | null;
  })[];
  execution: {
    source: string;
    channel: string;
    venue: string;
    orders: number;
    filled_orders: number;
    statuses: Counts;
  }[];
  diagnostics: { code: string; message: string }[];
  closed_trades: Trade[];
  rows: Row[];
  pagination: { offset: number; limit: number; total: number };
  limitations: string[];
};
const labels: Record<string, string> = {
  TARGET_HIT: "目标先触达",
  STOP_HIT: "止损先触达",
  STRONG_OPPOSITE_MOVE: "强反向波动",
  TIMEOUT: "超时",
  TIMEOUT_POSITIVE: "超时 · 正向",
  TIMEOUT_NEGATIVE: "超时 · 反向",
  DATA_INVALIDATED: "数据失效",
  SIGNAL_INVALIDATED: "信号失效",
  INVALIDATED: "旧记录失效",
  PENDING: "待完成",
  CLOSED: "已平仓",
  OPEN: "已有成交 · 未平仓",
  ORDER_PENDING: "订单待确认",
  UNFILLED: "未成交",
  MANUAL_SKIP: "人工跳过",
  OBSERVE_ONLY: "观察开场 · 未记录执行",
  ADMISSION_BLOCKED: "准入阻止",
  EXECUTION_BLOCKED: "执行受阻",
  NOT_QUALIFIED: "未通过资格",
  NO_EXECUTION_EVIDENCE: "未记录执行",
  priority: "优先机会",
  qualified: "合格机会",
  unverified: "待验证",
  blocked: "未达标",
  inactive: "信息不足",
  PAPER: "本地 PAPER",
  TESTNET: "交易所模拟",
  LIVE: "真实资金",
  UNKNOWN: "记录缺失",
  live: "真实行情",
  demo: "合成行情",
  confirmed_signal_change: "确认方向变化",
  market_data_integrity: "行情完整性异常",
  process_restart: "进程重启",
  calibration: "概率校准未通过",
  model_trust: "模型可信度不足",
  freshness: "行情过期",
  liquidity: "流动性不足",
  exchange: "场所数据不可用",
  net_ev: "成本后优势不足",
  net_pnl: "预计净收益不足",
  probability: "概率门槛未通过",
  regime: "市场状态校准不足",
  stale_feed: "行情过期",
  NEW: "已受理，尚未成交",
  QUEUED: "排队中",
  SUBMITTING: "提交中",
  PARTIALLY_FILLED: "部分成交",
  FILLED: "已成交",
  CANCELED: "已撤单",
  REJECTED: "已拒绝",
  EXPIRED: "已过期",
};
const label = (s: string) => labels[s] || s;
const num = (v: number | null | undefined, digits = 2) =>
  v == null
    ? "—"
    : v.toLocaleString("zh-CN", {
        maximumFractionDigits: digits,
        minimumFractionDigits: digits,
      });
const pct = (v: number | null | undefined) =>
  v == null ? "—" : `${(v * 100).toFixed(2)}%`;
const time = (ts: number) =>
  new Date(ts).toLocaleString("zh-CN", {
    timeZone: "Asia/Shanghai",
    hour12: false,
  });
const today = () =>
  new Date(Date.now() + 8 * 3600_000).toISOString().slice(0, 10);
function Breakdown({ title, counts }: { title: string; counts: Counts }) {
  return (
    <div>
      <h3>{title}</h3>
      {Object.keys(counts).length ? (
        <ul className="review-counts">
          {Object.entries(counts)
            .sort((a, b) => b[1] - a[1])
            .map(([k, v]) => (
              <li key={k}>
                <span>{label(k)}</span>
                <strong>{v}</strong>
              </li>
            ))}
        </ul>
      ) : (
        <p className="muted">暂无记录</p>
      )}
    </div>
  );
}
export default function ReviewPanel({ source }: { source: string }) {
  const [date, setDate] = useState(today);
  const [scope, setScope] = useState("day");
  const [symbol, setSymbol] = useState("");
  const [report, setReport] = useState<ReviewReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const requestId = useRef(0);
  const load = useCallback(
    async (offset = 0, snapshot?: ReviewReport, download = false) => {
      const token = ++requestId.current;
      setBusy(true);
      setError("");
      if (!snapshot && !download) setReport(null);
      const q = new URLSearchParams({
        date: scope === "all" ? "all" : date,
        source,
        offset: String(offset),
        limit: "50",
      });
      if (symbol.trim()) q.set("symbol", symbol.trim().toUpperCase());
      if (snapshot) {
        q.set("through_id", String(snapshot.through_id));
        q.set("as_of", String(snapshot.as_of));
      }
      if (download) q.set("export", "true");
      try {
        const response = await fetch(`/api/review?${q}`, { cache: "no-store" });
        const value = await response.json();
        if (!response.ok) throw new Error(value.error || "复盘读取失败");
        if (token !== requestId.current) return;
        if (download) {
          const blob = new Blob([JSON.stringify(value, null, 2)], {
            type: "application/json",
          });
          const url = URL.createObjectURL(blob);
          const a = document.createElement("a");
          a.href = url;
          a.download = `review-${source}-${scope === "all" ? "all" : date}.json`;
          a.click();
          URL.revokeObjectURL(url);
        } else setReport(value);
      } catch (e) {
        if (token === requestId.current)
          setError(e instanceof Error ? e.message : "复盘读取失败");
      } finally {
        if (token === requestId.current) setBusy(false);
      }
    },
    [date, scope, source, symbol],
  );
  useEffect(() => {
    void load();
    return () => {
      requestId.current++;
    };
  }, [load]);
  const s = report?.summary;
  return (
    <section
      className="panel universe-panel review-panel"
      data-testid="daily-review"
    >
      <div className="panel-header">
        <div>
          <h2>每日复盘与效果评估</h2>
          <p>北京时间 · {label(source)} · 预测结果与成交损益分别统计</p>
        </div>
      </div>
      <div className="universe-controls">
        <label>
          评估范围
          <select
            aria-label="复盘范围"
            value={scope}
            onChange={(e) => setScope(e.target.value)}
          >
            <option value="day">每日复盘</option>
            <option value="all">累计评估</option>
          </select>
        </label>
        {scope === "day" && (
          <label>
            复盘日期
            <input
              aria-label="复盘日期"
              type="date"
              value={date}
              max={today()}
              onChange={(e) => {
                if (e.target.value) setDate(e.target.value);
              }}
            />
          </label>
        )}
        <label>
          币种筛选
          <input
            aria-label="复盘币种"
            placeholder="全部币种 / BTCUSDT"
            value={symbol}
            onChange={(e) => setSymbol(e.target.value)}
          />
        </label>
        <button className="button" disabled={busy} onClick={() => void load()}>
          刷新复盘
        </button>
        <button
          className="button"
          disabled={busy || !report}
          onClick={() => void load(0, report!, true)}
        >
          导出完整 JSON
        </button>
      </div>
      {busy && <p role="status">正在读取完整历史并汇总…</p>}
      {error && (
        <p role="alert" className="review-error">
          {error}。请检查核心是否已升级并运行。
        </p>
      )}
      {report && s && (
        <>
          <p className="muted">
            报告截止：{time(report.as_of)} ·{" "}
            {report.provisional
              ? "进行中，结果尚未收齐"
              : "按当日结束时证据复盘"}
            。历史日报中的跨日结果可在累计评估查看。
          </p>
          <div className="universe-stats review-stats">
            {[
              ["正式机会", s.opportunities],
              ["曾通过资格", s.qualified],
              ["有效结果", s.eligible],
              ["失效结果", s.invalid],
              ["待完成", s.pending],
              ["PAPER 入场机会", s.paper_entries],
              ["交易所已成交机会", s.exchange_filled_opportunities],
              ["平均观察净优势", pct(s.observed_net_edge)],
            ].map(([title, v]) => (
              <div key={title}>
                <small>{title}</small>
                <strong>{v}</strong>
              </div>
            ))}
          </div>
          <div className="review-diagnostics">
            <h3>本期结论与待核查项</h3>
            <ul>
              {report.diagnostics.map((d) => (
                <li key={d.code}>{d.message}</li>
              ))}
            </ul>
          </div>
          <div className="review-grid">
            <Breakdown title="预测结果分布" counts={s.outcomes} />
            <Breakdown title="失效原因" counts={s.invalid_reasons} />
            <Breakdown title="执行去向（按机会去重）" counts={s.execution} />
            <Breakdown
              title="未执行相关原因（可多项）"
              counts={s.non_execution_reasons}
            />
          </div>
          <h3>已平仓交易效果</h3>
          <p className="muted">
            按平仓日期统计，包含此前开场的仓位。没有成交时收益显示“—”。
          </p>
          {!report.trading.length ? (
            <p className="empty">尚无已平仓成交验证</p>
          ) : (
            <div className="review-table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>数据 / 执行环境</th>
                    <th>已平仓</th>
                    <th>净损益 USDT</th>
                    <th>平均每笔</th>
                    <th>盈利比例</th>
                    <th>已平仓净损益回撤 USDT</th>
                    <th>平均成本超支 USDT</th>
                  </tr>
                </thead>
                <tbody>
                  {report.trading.map((g) => (
                    <tr key={`${g.source}-${g.channel}-${g.venue}`}>
                      <td>
                        {label(g.source)} / {label(g.channel)} / {g.venue}
                      </td>
                      <td>{g.metrics.closed_trades}</td>
                      <td>{num(g.metrics.net_pnl)}</td>
                      <td>{num(g.metrics.average_net_pnl)}</td>
                      <td>{pct(g.metrics.win_rate)}</td>
                      <td>{num(g.metrics.realized_pnl_drawdown)}</td>
                      <td>
                        {num(g.metrics.mean_cost_overrun)}（
                        {g.metrics.cost_samples} 笔）
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {!!report.closed_trades.length && (
            <details>
              <summary>
                平仓流水与版本归属（{report.closed_trades.length} 笔）
              </summary>
              <div className="review-table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th>平仓时间 / 币种</th>
                      <th>环境</th>
                      <th>模型 / 配置</th>
                      <th>净损益 USDT</th>
                      <th>退出原因 / 原始机会</th>
                    </tr>
                  </thead>
                  <tbody>
                    {report.closed_trades.map((t) => (
                      <tr key={`${t.source}-${t.channel}-${t.id}`}>
                        <td>
                          {time(t.closed_at)}
                          <br />
                          {t.symbol}
                        </td>
                        <td>
                          {label(t.source)} / {label(t.channel)}
                        </td>
                        <td>
                          {t.model_version} / {t.config_version ?? "未知"}
                        </td>
                        <td>{num(t.net_pnl)}</td>
                        <td>
                          {t.exit_reason}
                          <br />
                          {t.prediction_id}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </details>
          )}
          {!!report.trade_cohorts.length && (
            <details>
              <summary>成交效果按模型与配置版本分组</summary>
              <div className="review-table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th>币种 / 环境</th>
                      <th>模型 / 配置</th>
                      <th>已平仓</th>
                      <th>净损益 USDT</th>
                      <th>平均每笔 USDT</th>
                    </tr>
                  </thead>
                  <tbody>
                    {report.trade_cohorts.map((c, i) => (
                      <tr key={i}>
                        <td>
                          {c.symbol} / {label(c.source)} / {label(c.channel)} /{" "}
                          {c.venue}
                        </td>
                        <td>
                          {c.model_version} / {c.config_version ?? "未知"}
                        </td>
                        <td>{c.metrics.closed_trades}</td>
                        <td>{num(c.metrics.net_pnl)}</td>
                        <td>{num(c.metrics.average_net_pnl)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </details>
          )}
          {!!report.execution.length && (
            <details>
              <summary>订单执行统计（期间创建或更新，按订单去重）</summary>
              {report.execution.map((g) => (
                <Breakdown
                  key={`${g.source}-${g.channel}-${g.venue}`}
                  title={`${label(g.source)} / ${label(g.channel)} / ${g.venue} · ${g.orders} 笔订单，${g.filled_orders} 笔有成交`}
                  counts={g.statuses}
                />
              ))}
            </details>
          )}
          <details>
            <summary>
              按币种、模型与配置版本比较（{report.cohorts.length} 组）
            </summary>
            <div className="review-table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>币种 / 来源</th>
                    <th>模型</th>
                    <th>配置版本</th>
                    <th>机会 / 有效 / 失效</th>
                    <th>目标触达率</th>
                    <th>校准误差 / 样本</th>
                    <th>平均观察净优势</th>
                  </tr>
                </thead>
                <tbody>
                  {report.cohorts.map((c) => (
                    <tr
                      key={`${c.source}-${c.symbol}-${c.model_version}-${c.config_version}`}
                    >
                      <td>
                        {c.symbol} / {label(c.source)}
                      </td>
                      <td>{c.model_version}</td>
                      <td>{c.config_version}</td>
                      <td>
                        {c.metrics.opportunities} / {c.metrics.eligible} /{" "}
                        {c.metrics.invalid}
                      </td>
                      <td>{pct(c.metrics.target_hit_rate)}</td>
                      <td>
                        {pct(c.metrics.calibration_ece)} /{" "}
                        {c.metrics.calibration_samples}
                      </td>
                      <td>{pct(c.metrics.observed_net_edge)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <p className="muted">
              目标触达率仅以有效 First-hit
              结果为分母，不能代表全部机会或成交胜率。校准误差仅使用有效且有开场校准概率的样本。不同版本结果不能直接解释为升级改善。
            </p>
          </details>
          <h3>机会流水 · 共 {report.pagination.total} 条</h3>
          {!report.rows.length ? (
            <p className="empty">该范围暂无正式机会</p>
          ) : (
            <div className="review-table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>开场 / 币种</th>
                    <th>开场等级</th>
                    <th>预测结果</th>
                    <th>执行去向</th>
                    <th>证据与成交结果</th>
                  </tr>
                </thead>
                <tbody>
                  {report.rows.map((r) => (
                    <tr key={`${r.source}-${r.prediction_id}`}>
                      <td>
                        {time(r.opened_at)}
                        <br />
                        <strong>
                          {r.symbol} · {r.direction}
                        </strong>
                      </td>
                      <td>
                        <span
                          className={`opportunity-badge opportunity-${r.opening_grade}`}
                        >
                          {label(r.opening_grade)}
                        </span>
                      </td>
                      <td>
                        {label(r.outcome || "PENDING")}
                        <br />
                        <small>
                          {r.learning_eligible
                            ? "可学习"
                            : r.outcome
                              ? "不纳入学习"
                              : "等待结束"}
                        </small>
                      </td>
                      <td>{label(r.execution)}</td>
                      <td>
                        <details>
                          <summary>展开证据</summary>
                          <p>
                            {r.prediction_id}
                            <br />
                            模型 {r.model_version} / 配置 {r.config_version}
                          </p>
                          {r.invalid_reason && (
                            <p>结束原因：{label(r.invalid_reason)}</p>
                          )}
                          {r.reasons.map((reason) => (
                            <p key={reason}>{label(reason)}</p>
                          ))}
                          {r.shadow_admission && (
                            <p>
                              仅影子准入：{r.shadow_admission.decision}
                              （不代表执行）
                            </p>
                          )}
                          {r.orders.map((o) => (
                            <p key={o.id}>
                              {label(o.channel)} / {o.venue}：{o.status}，成交量{" "}
                              {num(o.filled_qty, 6)}
                            </p>
                          ))}
                          {r.closed_trades.map((t) => (
                            <p key={`${t.channel}-${t.id}`}>
                              {label(t.channel)} 已平仓净损益 {num(t.net_pnl)}{" "}
                              USDT；退出：{t.exit_reason}
                            </p>
                          ))}
                          {r.horizon_observation && (
                            <p>
                              独立到期观察：
                              {r.horizon_observation.status === "OBSERVED"
                                ? pct(
                                    r.horizon_observation.final_move_at_timeout,
                                  )
                                : "数据不可用"}
                              ，不改写原标签或计为成交收益。
                            </p>
                          )}
                        </details>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          <div className="universe-controls">
            <button
              className="button"
              disabled={busy || report.pagination.offset === 0}
              onClick={() =>
                void load(Math.max(0, report.pagination.offset - 50), report)
              }
            >
              上一页
            </button>
            <span>
              第 {Math.floor(report.pagination.offset / 50) + 1} /{" "}
              {Math.max(1, Math.ceil(report.pagination.total / 50))} 页
            </span>
            <button
              className="button"
              disabled={
                busy || report.pagination.offset + 50 >= report.pagination.total
              }
              onClick={() => void load(report.pagination.offset + 50, report)}
            >
              下一页
            </button>
          </div>
          <details>
            <summary>统计口径与自动日报</summary>
            <ul>
              {report.limitations.map((l) => (
                <li key={l}>{l}</li>
              ))}
            </ul>
            <p>
              核心运行期间每 5 分钟检查一次，自动保存上一北京时间自然日的 JSON
              日报；重启补上一日，重复运行不覆盖已归档报告。长期停机期间的更早日期可在此查询并导出。文件位于当前数据目录的
              reviews/live/ 或 reviews/demo/，按行情来源隔离。
            </p>
          </details>
        </>
      )}
    </section>
  );
}
