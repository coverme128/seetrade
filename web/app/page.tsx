"use client";
import { AdmissionCard, IntegrityPanel } from "./admission-panel";
import OperationsPanel from "./operations-panel";
import { useCallback, useEffect, useState } from "react";
import { ShadowPanel } from "./shadow-panel";
import type {
  Config,
  Health,
  Outcome,
  Prediction,
  State,
  SymbolState,
  Trade,
} from "./types";

const money = (v: number | undefined | null, digits = 2) =>
  v == null
    ? "—"
    : new Intl.NumberFormat("en-US", {
        minimumFractionDigits: digits,
        maximumFractionDigits: digits,
      }).format(v);
const pct = (v: number | undefined | null, digits = 2) =>
  v == null ? "—" : `${(v * 100).toFixed(digits)}%`;
const signed = (v: number, suffix = "") =>
  `${v >= 0 ? "+" : ""}${money(v)}${suffix}`;
const time = (v: number) =>
  new Date(v).toLocaleTimeString("zh-CN", { hour12: false });
const outcomes: Record<string, string> = {
  TARGET_HIT: "目标先触达",
  STOP_HIT: "止损先触达",
  TIMEOUT_POSITIVE: "超时 · 正向",
  TIMEOUT_NEGATIVE: "超时 · 反向",
  STRONG_OPPOSITE_MOVE: "强反向波动",
  INVALIDATED: "数据失效（旧记录）",
  DATA_INVALIDATED: "数据失效",
  SIGNAL_INVALIDATED: "信号失效",
};
const modes: Record<string, string> = {
  OBSERVE: "观察",
  PAPER: "模拟",
  CONFIRM: "确认交易",
  AUTO: "自动交易",
};
function Icon({ name, size = 20 }: { name: string; size?: number }) {
  const paths: Record<string, React.ReactNode> = {
    grid: (
      <>
        <rect x="3" y="3" width="7" height="7" rx="1.5" />
        <rect x="14" y="3" width="7" height="7" rx="1.5" />
        <rect x="3" y="14" width="7" height="7" rx="1.5" />
        <rect x="14" y="14" width="7" height="7" rx="1.5" />
      </>
    ),
    history: (
      <>
        <path d="M3 11a9 9 0 1 1 2.7 7M3 4v7h7" />
        <path d="M12 7v5l3 2" />
      </>
    ),
    pulse: <path d="M2 12h4l3-7 5 14 3-7h5" />,
    settings: (
      <>
        <path d="M4 7h16M4 17h16" />
        <circle cx="9" cy="7" r="3" />
        <circle cx="16" cy="17" r="3" />
      </>
    ),
    arrow: <path d="M5 17 17 5M5 5h12v12" />,
    shield: (
      <>
        <path d="m12 3 8 3v6c0 5-8 9-8 9s-8-4-8-9V6Z" />
        <path d="m8 12 3 3 5-6" />
      </>
    ),
    stop: (
      <>
        <circle cx="12" cy="12" r="9" />
        <rect x="9" y="9" width="6" height="6" rx="1" />
      </>
    ),
    chevron: <path d="m9 5 7 7-7 7" />,
    check: <path d="m5 12 4 4 10-10" />,
    close: <path d="m6 6 12 12M6 18 18 6" />,
    download: (
      <>
        <path d="M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5" />
      </>
    ),
  };
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {paths[name] || paths.pulse}
    </svg>
  );
}
function Badge({
  children,
  tone = "muted",
}: {
  children: React.ReactNode;
  tone?: string;
}) {
  return <span className={`badge ${tone}`}>{children}</span>;
}
function Empty({ children }: { children: React.ReactNode }) {
  return (
    <div className="empty">
      <Icon name="pulse" size={28} />
      <p>{children}</p>
    </div>
  );
}
function Chart({ s, p }: { s: SymbolState; p: Prediction | null }) {
  const data = s.chart;
  if (data.length < 2) return <Empty>等待连续行情，价格轨迹将在这里出现</Empty>;
  const W = 920,
    H = 225,
    pad = 22;
  const values = data.map((x) => x[1]);
  const lo = Math.min(...values),
    hi = Math.max(...values);
  const margin = Math.max((hi - lo) * 0.2, lo * 0.00008);
  const low = lo - margin,
    high = hi + margin;
  const y = (v: number) => H - pad - ((v - low) / (high - low)) * (H - pad * 2);
  const x = (i: number) => (i / (data.length - 1)) * (W - 82) + 8;
  const path = data
    .map((d, i) => `${i ? "L" : "M"}${x(i).toFixed(1)},${y(d[1]).toFixed(1)}`)
    .join(" ");
  const last = data.at(-1)!;
  return (
    <div className="chart">
      <svg
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label="最近三分钟中间价走势"
      >
        <defs>
          <linearGradient id="price-area" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#54dbbd" stopOpacity=".19" />
            <stop offset="100%" stopColor="#54dbbd" stopOpacity="0" />
          </linearGradient>
        </defs>
        {[0, 1, 2, 3].map((i) => {
          const value = low + ((high - low) * i) / 3;
          return (
            <g key={i}>
              <line
                x1="0"
                x2={W - 76}
                y1={y(value)}
                y2={y(value)}
                stroke="#ffffff0b"
                strokeDasharray="3 5"
              />
              <text x={W - 65} y={y(value) + 4} className="chart-label">
                {money(value, 1)}
              </text>
            </g>
          );
        })}
        {p && p.start_price > low && p.start_price < high && (
          <>
            <line
              x1="0"
              x2={W - 76}
              y1={y(p.start_price)}
              y2={y(p.start_price)}
              stroke="#c3b37a66"
              strokeDasharray="4 4"
            />
            <text x="12" y={y(p.start_price) - 8} className="chart-label">
              预测起点
            </text>
          </>
        )}
        <path
          d={`${path} L${x(data.length - 1)},${H} L8,${H} Z`}
          fill="url(#price-area)"
        />
        <path d={path} fill="none" stroke="#6ce4c5" strokeWidth="2" />
        <circle cx={x(data.length - 1)} cy={y(last[1])} r="4" fill="#a6f7de" />
      </svg>
      <div className="chart-times">
        <span>{time(data[0][0])}</span>
        <span>BINANCE · MID PRICE · 1s</span>
        <span>{time(last[0])}</span>
      </div>
    </div>
  );
}
function HealthBars({ health }: { health: Health }) {
  return (
    <div className="calibration-chart">
      <div className="calibration-bars">
        {health.bins.map((b) => (
          <div
            className="bin"
            key={b.lower}
            title={`${b.count} 个样本 · 预测 ${pct(b.predicted)} / 实际 ${pct(b.actual)}`}
          >
            <div className="bar-pair">
              <i style={{ height: `${b.predicted * 100}%` }} />
              <i style={{ height: `${b.actual * 100}%` }} />
            </div>
            <span>{Math.round(b.lower * 100)}</span>
          </div>
        ))}
      </div>
      <div className="chart-times">
        <span>概率分桶 %</span>
        <span>
          <i className="legend-dot mint" />
          预测 <i className="legend-dot blue" />
          实际
        </span>
      </div>
    </div>
  );
}

export default function Home() {
  const [state, setState] = useState<State | null>(null),
    [connected, setConnected] = useState(false),
    [lastSync, setLastSync] = useState(0);
  const [tab, setTab] = useState("overview"),
    [symbol, setSymbol] = useState("BTCUSDT"),
    [toast, setToast] = useState(""),
    [busy, setBusy] = useState(false);
  const [selected, setSelected] = useState<Outcome | null>(null),
    [annotation, setAnnotation] = useState(""),
    [trade, setTrade] = useState<Trade | null>(null);
  const [historyKind, setHistoryKind] = useState("prediction"),
    [historyRows, setHistoryRows] = useState<Outcome[] | null>(null);
  const [draft, setDraft] = useState<Config | null>(null),
    [gatesExpanded, setGatesExpanded] = useState(false);
  useEffect(() => {
    if (tab === "settings" && state && !draft) setDraft({ ...state.config });
  }, [tab, state, draft]);
  const refresh = useCallback(async () => {
    try {
      const r = await fetch("/api/state", {
        cache: "no-store",
        signal: AbortSignal.timeout(3000),
      });
      if (!r.ok) throw new Error();
      const data: State = await r.json();
      setState(data);
      setConnected(true);
      setLastSync(Date.now());
    } catch {
      setConnected(false);
    }
  }, []);
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      if (!active) return;
      await refresh();
      if (active) timer = setTimeout(poll, 1000);
    };
    poll();
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [refresh]);
  useEffect(() => {
    if (!toast) return;
    const id = setTimeout(() => setToast(""), 6000);
    return () => clearTimeout(id);
  }, [toast]);
  useEffect(() => {
    const close = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setSelected(null);
        setTrade(null);
      }
    };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, []);
  const command = async (name: string, payload: unknown) => {
    if (!connected || busy) return;
    setBusy(true);
    try {
      const r = await fetch(`/api/control/${name}`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Micro-Alpha": "local",
        },
        body: JSON.stringify(payload),
      });
      const body = await r.json();
      if (!r.ok) throw new Error(body.error || "操作失败");
      setState(body);
      setToast("已保存，操作已记录到审计日志");
      return true;
    } catch (e) {
      setToast(e instanceof Error ? e.message : "无法连接核心服务");
      return false;
    } finally {
      setBusy(false);
    }
  };
  const openHistory = async () => {
    try {
      const r = await fetch("/api/history?kind=outcome&limit=500");
      if (!r.ok) throw new Error();
      const rows = await r.json();
      setHistoryRows(rows.map((r: { payload: Outcome }) => r.payload));
      setToast("已加载最近 500 条持久化结果");
    } catch {
      setToast("历史记录读取失败");
    }
  };
  const exportData = () => {
    if (!state) return;
    const blob = new Blob(
      [
        JSON.stringify(
          {
            source: state.source,
            exported_at: new Date().toISOString(),
            outcomes: historyRows || state.outcomes,
            trades: state.trades,
          },
          null,
          2,
        ),
      ],
      { type: "application/json" },
    );
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `micro-alpha-${state.source}-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(a.href);
  };
  const s = state?.symbols[symbol],
    p = s?.active,
    q = s?.qualification,
    plan = s?.plan,
    f = s?.features,
    h = s?.health;
  const card = p || s?.last_result?.prediction;
  const result = !p ? s?.last_result : null;
  const probs = p
    ? p.episode?.latest_calibrated_probabilities ||
      p.episode?.latest_raw_probabilities ||
      p.calibrated_probabilities ||
      p.raw_probabilities
    : card?.calibrated_probabilities || card?.raw_probabilities;
  const position = state?.positions[symbol];
  const isDemo = state?.source === "demo";
  const displayedOutcomes = (historyRows || state?.outcomes || []).filter(
    (o) => o.prediction.symbol === symbol,
  );
  const go = (next: string) => {
    setTab(next);
    if (next === "settings" && state) setDraft({ ...state.config });
  };
  const titles: Record<string, [string, string]> = {
    overview: ["市场工作台", "从市场结构到可验证的交易判断。"],
    history: ["预测与交易记录", "让每一次判断，都有可追溯的结果。"],
    health: ["模型健康", "先验证可靠性，再分配风险。"],
    settings: ["个人风险设置", "约束先于执行，每次变更留有记录。"],
  };
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a className="brand" href="/" aria-label="Micro Alpha 首页">
          <span className="brand-icon">
            <svg viewBox="0 0 28 28" fill="none">
              <path
                d="M3 22 10 6l5 11 4-8 6 13"
                stroke="currentColor"
                strokeWidth="2.5"
              />
              <path d="M3 22h22" stroke="currentColor" strokeOpacity=".3" />
            </svg>
          </span>
          <span>
            micro<span className="brand-light">alpha</span>
            <small>PERSONAL TERMINAL</small>
          </span>
        </a>
        <div className="nav-label">工作空间</div>
        <nav>
          {[
            ["overview", "grid", "市场工作台"],
            ["history", "history", "预测与交易"],
            ["health", "pulse", "模型健康"],
            ["settings", "settings", "风险设置"],
          ].map(([id, icon, label]) => (
            <button
              className={`nav-item ${tab === id ? "active" : ""}`}
              key={id}
              aria-label={label}
              onClick={() => go(id)}
            >
              <Icon name={icon} />
              <span>{label}</span>
              {id === "overview" && <span className="nav-dot" />}
            </button>
          ))}
        </nav>
        <div className="sidebar-note">
          <Icon name="shield" />
          <strong>纪律，让判断更有价值。</strong>
          <p>
            正的成本后预期收益
            <br />
            明确的风险预算
            <br />
            持续的真实结果验证
          </p>
          <span>
            AI ASSIST <b>OFF</b>
          </span>
        </div>
        <div className="sidebar-bottom">
          <div className="avatar">P</div>
          <div>
            个人工作空间<small>Local · USDT 永续合约</small>
          </div>
          <span className="status-dot" />
        </div>
      </aside>
      <div className="main-shell">
        <header className="topbar">
          <div className="breadcrumb">
            工作空间 <span>/</span> <b>{titles[tab][0]}</b>
          </div>
          <div className="top-status">
            <span className={`status-dot ${connected ? "" : "bad"}`} />
            <span>{connected ? "核心已连接" : "核心未连接"}</span>
            <span className="divider" />
            <span className="mono">{state ? time(state.now) : "--:--:--"}</span>
            <span className="timezone">LOCAL</span>
          </div>
        </header>
        <main>
          <div className="page-heading">
            <div>
              <div className="eyebrow">
                MICROSTRUCTURE INTELLIGENCE <span> / 01</span>
              </div>
              <h1>{titles[tab][0]}</h1>
              <p>{titles[tab][1]}</p>
            </div>
            <button
              className={`kill-button ${state?.kill_switch ? "engaged" : ""}`}
              disabled={!connected || busy}
              onClick={() => command("kill", { active: !state?.kill_switch })}
            >
              <Icon name="stop" size={16} />
              {state?.kill_switch ? "解除紧急停止" : "紧急停止"}
            </button>
          </div>
          {!connected && (
            <div className="notice error">
              <Icon name="pulse" size={17} />
              <span>
                无法连接本地核心。请运行 <code>./scripts/dev.sh --demo</code> 或{" "}
                <code>./scripts/dev.sh</code>。
                {lastSync > 0 &&
                  ` 最后更新 ${time(lastSync)}，下方保留的是旧数据。`}
              </span>
            </div>
          )}
          {isDemo && (
            <div className="notice">
              <span className="demo-tag">DEMO</span>
              <span>
                当前为合成行情演示。预测、校准与模拟收益均不代表真实市场表现。
              </span>
              <span className="notice-tail">独立数据空间</span>
            </div>
          )}
          {state?.kill_switch && (
            <div className="notice error">
              <Icon name="stop" size={17} />
              <span>
                紧急停止已生效：已停止新增订单。持仓策略：
                {state.config.kill_behavior === "hold"
                  ? "保留持仓，继续 TP / SL / 超时退出"
                  : "按有效报价平仓，断流时等待恢复"}
                。
              </span>
            </div>
          )}
          {state && !state.storage_healthy && (
            <div className="notice error">
              存储异常：已停止新增订单。检查磁盘空间与核心日志。
            </div>
          )}
          <div className="workspace-bar">
            <div className="symbol-tabs">
              {["BTCUSDT", "ETHUSDT"].map((v, i) => (
                <button
                  aria-label={`${v} 永续合约`}
                  onClick={() => setSymbol(v)}
                  className={symbol === v ? "selected" : ""}
                  key={v}
                >
                  <span className={`coin coin-${i}`}>
                    {i === 0 ? "₿" : "Ξ"}
                  </span>
                  <span>
                    {v.replace("USDT", "")}
                    <small>/ USDT</small>
                  </span>
                  <span className="perp">PERP</span>
                </button>
              ))}
            </div>
            <div className="mode-select">
              {Object.entries(modes).map(([mode, label]) => (
                <button
                  key={mode}
                  className={state?.mode === mode ? "selected" : ""}
                  disabled={
                    !connected ||
                    busy ||
                    (["CONFIRM", "AUTO"].includes(mode) &&
                      !state?.live_enabled) ||
                    (mode === "AUTO" && !state?.auto_evidence_ready)
                  }
                  title={
                    ["CONFIRM", "AUTO"].includes(mode)
                      ? state?.live_gate_reason || undefined
                      : label
                  }
                  onClick={() => {
                    if (
                      mode !== "AUTO" ||
                      window.confirm(
                        "开启 AUTO 后将按风险门槛自动执行真实账户订单，是否启用？",
                      )
                    )
                      command("mode", {
                        mode,
                        acknowledge_auto: mode === "AUTO",
                      });
                  }}
                >
                  {mode}
                  <small>
                    {label}
                    {["CONFIRM", "AUTO"].includes(mode) && !state?.live_enabled
                      ? " · 未就绪"
                      : ""}
                  </small>
                </button>
              ))}
            </div>
          </div>
          {!state || !s ? (
            <div className="panel initial-empty">
              <Empty>等待交易核心连接。界面不会用模拟数值填补缺失行情。</Empty>
            </div>
          ) : (
            <>
              {(tab === "health" ||
                tab === "settings" ||
                state.live?.configured) && (
                <OperationsPanel
                  state={state}
                  symbol={symbol}
                  busy={busy}
                  command={command}
                />
              )}
              {tab === "overview" && (
                <>
                  <div className="metric-strip">
                    <div className="market-price">
                      <label>
                        {symbol} <Badge tone="muted">永续</Badge>
                      </label>
                      <strong className="mono">${money(f?.mid)}</strong>
                      <span>
                        {f ? (
                          <>
                            <span
                              className={
                                f.momentum >= 0 ? "positive" : "negative"
                              }
                            >
                              {signed(f.momentum * 100, "%")}
                            </span>{" "}
                            <span className="muted">近 10 秒</span>
                          </>
                        ) : (
                          "等待 Binance 深度同步"
                        )}
                      </span>
                    </div>
                    <div>
                      <label>
                        价差 <span className="tiny">SPREAD</span>
                      </label>
                      <strong className="mono">
                        {money(f?.spread_bps)} <small>bps</small>
                      </strong>
                      <span className="muted">买一 / 卖一</span>
                    </div>
                    <div>
                      <label>近端流动性</label>
                      <strong className="mono">
                        {f ? `$${money(f.depth / 1000, 1)}k` : "—"}
                      </strong>
                      <span
                        className={
                          f && f.depth >= state.config.min_depth
                            ? "positive"
                            : "muted"
                        }
                      >
                        {f
                          ? f.depth >= state.config.min_depth
                            ? "深度充足"
                            : "深度不足"
                          : "等待数据"}
                      </span>
                    </div>
                    <div>
                      <label>短期波动</label>
                      <strong className="mono">{pct(f?.volatility, 3)}</strong>
                      <span className="muted">1s realized volatility</span>
                    </div>
                    <div className="exchange-health">
                      <label>行情连接</label>
                      {Object.entries(s.venues).map(([name, v]) => (
                        <div
                          key={name}
                          title={`${v.error} · 本机接收 ${v.age_ms}ms · 交易所时间 ${v.exchange_age_ms}ms`}
                        >
                          <span
                            className={`status-dot ${v.ready ? "" : "bad"}`}
                          />
                          <b>{name === "binance" ? "Binance" : "OKX"}</b>
                          <span className="mono muted">
                            {v.ready
                              ? `${v.exchange_age_ms}ms`
                              : v.connected
                                ? "过期 / 同步中"
                                : "未连接"}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                  <div className="workspace-grid">
                    <section className="panel market-panel">
                      <div className="panel-heading">
                        <div>
                          <span className="section-index">01</span>
                          <h2>市场脉搏</h2>
                          <span className="caption">LIVE MARKET</span>
                        </div>
                        <Badge
                          tone={
                            f?.data_quality.length === 0
                              ? "positive"
                              : "warning"
                          }
                        >
                          {f?.data_quality.length === 0
                            ? "数据连续"
                            : "等待同步"}
                        </Badge>
                      </div>
                      <Chart s={s} p={p || null} />
                      <div className="drivers">
                        <div className="drivers-title">
                          <span>WHY NOW</span>
                          <small>当前结构驱动</small>
                        </div>
                        {[
                          ["订单簿失衡", f?.book_imbalance, "book"],
                          ["主动成交", f?.aggressive_imbalance, "trade"],
                          ["微价格偏移", f?.microprice_bps, "micro"],
                          ["跨市场价差", f?.cross_venue_bps, "cross"],
                        ].map(([label, value, key]) => {
                          const v = typeof value === "number" ? value : null;
                          return (
                            <div className="driver" key={String(key)}>
                              <label>{label}</label>
                              <strong
                                className={
                                  v === null
                                    ? "muted"
                                    : Math.abs(v) < 0.03
                                      ? "muted"
                                      : v > 0
                                        ? "positive"
                                        : "negative"
                                }
                              >
                                {v === null
                                  ? "等待"
                                  : key === "cross"
                                    ? Math.abs(v) <
                                      state.config.max_cross_venue_bps
                                      ? "一致"
                                      : "偏离"
                                    : Math.abs(v) < 0.03
                                      ? "中性"
                                      : v > 0
                                        ? "买方占优 ↗"
                                        : "卖方占优 ↘"}
                              </strong>
                              <span className="mono muted">
                                {v === null
                                  ? "—"
                                  : key === "micro" || key === "cross"
                                    ? `${money(v)} bps`
                                    : pct(v, 1)}
                              </span>
                            </div>
                          );
                        })}
                      </div>
                      <details className="raw-metrics">
                        <summary>展开原始特征与行情信息</summary>
                        {card?.episode?.origin_trigger_id && (
                          <p>
                            机会来源：
                            {card.episode.origin_trigger_families?.join(
                              " + ",
                            )}{" "}
                            · {card.episode.origin_trigger_id}
                          </p>
                        )}
                        <div className="detail-grid">
                          {[
                            ["订单流失衡", f?.flow_imbalance],
                            ["10s 动量", f?.momentum],
                            ["跨市场 1 秒收益差 bps", f?.cross_venue_lead_bps],
                            ["标记价格", f?.mark],
                            ["指数价格", f?.index],
                          ].map(([k, v]) => (
                            <div key={String(k)}>
                              <span>{k}</span>
                              <b className="mono">
                                {typeof v === "number" ? money(v, 5) : "—"}
                              </b>
                            </div>
                          ))}
                        </div>
                        <p>
                          跨市场收益差为 OKX 减 Binance 的 1 秒收益。
                          接收时间领先 / 滞后：
                          {f?.cross_venue_lag
                            ? `${f.cross_venue_lag.leader} · ${f.cross_venue_lag.lag_ms} ms · 相关系数 ${f.cross_venue_lag.correlation.toFixed(2)} · ${f.cross_venue_lag.samples} 对样本`
                            : "暂无足够的新鲜、相关样本"}
                          。用于行情背景，未接入模型打分。
                        </p>
                      </details>
                    </section>
                    <section className="panel recommendation">
                      <div className="panel-heading">
                        <div>
                          <span className="section-index">02</span>
                          <h2>核心判断</h2>
                        </div>
                        <span className="caption">CORE VIEW</span>
                      </div>
                      {card && probs ? (
                        <>
                          <div className="direction-row">
                            <div
                              className={`direction ${card.direction === "LONG" ? "positive" : "negative"}`}
                            >
                              <Icon name="arrow" size={31} />
                              {card.direction}
                              <span>
                                {card.direction === "LONG" ? "看多" : "看空"}
                              </span>
                            </div>
                            <Badge tone="muted">
                              {result ? "已完成 · 上次判断" : "跟踪中"}
                            </Badge>
                          </div>
                          <div className="prediction-sentence">
                            {result ? "原预测" : "未来"}{" "}
                            <b className="mono">{card.horizon}s</b> 内<br />
                            <strong>
                              {card.direction === "LONG" ? "+" : "−"}
                              {pct(card.target)}
                            </strong>{" "}
                            <span>先于</span>{" "}
                            <b>
                              {card.direction === "LONG" ? "−" : "+"}
                              {pct(card.stop)}
                            </b>
                          </div>
                          <div className="probability-bar">
                            {probs.map((v, i) => (
                              <span key={i} style={{ width: `${v * 100}%` }} />
                            ))}
                          </div>
                          <div className="probability-labels">
                            <div>
                              <label>
                                <i className="legend-dot mint" />
                                先涨
                              </label>
                              <b className="mono">{pct(probs[0], 1)}</b>
                            </div>
                            <div>
                              <label>
                                <i className="legend-dot red" />
                                先跌
                              </label>
                              <b className="mono">{pct(probs[1], 1)}</b>
                            </div>
                            <div>
                              <label>
                                <i className="legend-dot gray" />
                                超时
                              </label>
                              <b className="mono">{pct(probs[2], 1)}</b>
                            </div>
                          </div>
                          <div className="key-values padded">
                            <div>
                              <span>预期方向幅度（First-hit 近似）</span>
                              <b>
                                {card.expected_move == null
                                  ? "等待校准"
                                  : pct(card.expected_move)}
                              </b>
                            </div>
                            <div>
                              <span>信号强度（非概率）</span>
                              <b>{pct(card.signal_strength ?? 0, 1)}</b>
                            </div>
                          </div>
                          <div
                            className="padded"
                            data-testid="episode-snapshots"
                          >
                            <small className="mono">
                              Episode · {card.prediction_id}
                            </small>
                            <p className="muted">
                              {result
                                ? "开场概率"
                                : "上方显示最新概率；开场概率"}
                              ：
                              {(
                                card.calibrated_probabilities ||
                                card.raw_probabilities
                              )
                                .map((v) => pct(v, 1))
                                .join(" / ")}
                              {p?.episode?.latest_evaluated_at
                                ? ` · 更新于 ${time(p.episode.latest_evaluated_at)}`
                                : ""}
                            </p>
                          </div>
                          <div className="prediction-footer">
                            <span className="status-dot" />
                            <span>
                              {card.calibrated_probabilities
                                ? isDemo
                                  ? "演示校准 · 非真实验证"
                                  : "冻结模型 · 已校准"
                                : "原始概率 · 尚未校准"}
                            </span>
                            <span className="mono">{card.model_version}</span>
                          </div>
                          {result && (
                            <div
                              className="key-values padded"
                              data-testid="prediction-result"
                            >
                              <div>
                                <span>自动结果</span>
                                <b>
                                  {outcomes[result.outcome_type] ||
                                    result.outcome_type}
                                </b>
                              </div>
                              <div>
                                <span>实际方向收益 / 原目标</span>
                                <b>
                                  {pct(result.final_move)} / {pct(card.target)}
                                </b>
                              </div>
                              <div>
                                <span>MFE / MAE</span>
                                <b>
                                  {pct(result.mfe)} / {pct(result.mae)}
                                </b>
                              </div>
                              <div>
                                <span>学习资格</span>
                                <b>
                                  {result.learning_eligible
                                    ? "纳入自动样本"
                                    : "数据失效 · 排除"}
                                </b>
                              </div>
                              <button
                                type="button"
                                className="text-button"
                                onClick={() => setSelected(result)}
                              >
                                查看结果详情
                              </button>
                            </div>
                          )}
                        </>
                      ) : (
                        <Empty>
                          {s.signal_direction === "FLAT"
                            ? "FLAT · 中性观察，等待信号离开中性区间"
                            : "等待方向持续稳定，随后创建正式预测"}
                        </Empty>
                      )}
                      <p
                        className="panel-note"
                        data-testid="prediction-lifecycle"
                      >
                        当前状态 {s.lifecycle} · 候选方向 {s.signal_direction}
                        。结果保留至下一次正式预测，原始判断不会重写。
                      </p>
                    </section>
                    <section className="panel qualification">
                      <div className="panel-heading">
                        <div>
                          <span className="section-index">03</span>
                          <h2>交易资格</h2>
                          <span className="caption">QUALIFICATION</span>
                        </div>
                        <Badge
                          tone={
                            q?.status === "QUALIFIED" ? "positive" : "warning"
                          }
                        >
                          {q?.status === "QUALIFIED"
                            ? "QUALIFIED"
                            : q
                              ? "NOT QUALIFIED"
                              : "WATCH"}
                        </Badge>
                      </div>
                      <AdmissionCard state={state} symbol={symbol} />
                      {q ? (
                        <>
                          <div className="ev-summary">
                            <div>
                              <label>成本后预期优势</label>
                              <strong
                                className={`mono ${q.net_edge > 0 ? "positive" : "negative"}`}
                              >
                                {signed(q.net_edge * 100, "%")}
                              </strong>
                            </div>
                            <span className="ev-formula">
                              <b className="mono">{pct(q.gross_edge, 3)}</b>
                              <small>毛收益</small>
                              <span>−</span>
                              <b className="mono">{pct(q.costs.total, 3)}</b>
                              <small>总成本</small>
                            </span>
                          </div>
                          <div className="gates">
                            {q.gates
                              .filter(
                                (g, i) => gatesExpanded || !g.pass || i < 3,
                              )
                              .map((g) => (
                                <div className="gate-row" key={g.code}>
                                  <span
                                    className={`gate-icon ${g.pass ? "positive" : "warning"}`}
                                  >
                                    <Icon
                                      name={g.pass ? "check" : "close"}
                                      size={13}
                                    />
                                  </span>
                                  <span>{g.name}</span>
                                  <small title={g.detail}>{g.detail}</small>
                                  <b
                                    className={g.pass ? "positive" : "warning"}
                                  >
                                    {g.pass ? "PASS" : "WAIT"}
                                  </b>
                                </div>
                              ))}
                          </div>
                          <button
                            className="text-button"
                            onClick={() => setGatesExpanded(!gatesExpanded)}
                          >
                            {gatesExpanded
                              ? "收起检查项"
                              : `查看全部 ${q.gates.length} 项检查`}{" "}
                            <Icon name="chevron" size={12} />
                          </button>
                          <details className="raw-metrics">
                            <summary>成本假设明细</summary>
                            <div className="detail-grid">
                              {Object.entries(q.costs)
                                .filter(([k]) => k !== "total")
                                .map(([k, v]) => (
                                  <div key={k}>
                                    <span>
                                      {
                                        {
                                          spread: "价差",
                                          entry_fee: "入场费",
                                          exit_fee: "出场费",
                                          slippage: "双边滑点",
                                          impact: "市场冲击",
                                          adverse_selection: "逆向选择",
                                          funding: "资金费预留",
                                        }[k]
                                      }
                                    </span>
                                    <b className="mono">
                                      {money(v * 10000)} bps
                                    </b>
                                  </div>
                                ))}
                            </div>
                          </details>
                        </>
                      ) : (
                        <Empty>预测形成后，逐项检查交易条件</Empty>
                      )}
                    </section>
                    <section className="panel next-action">
                      <div className="panel-heading">
                        <div>
                          <span className="section-index">04</span>
                          <h2>下一步行动</h2>
                        </div>
                        <Icon name="shield" size={18} />
                      </div>
                      {plan ? (
                        <>
                          <div className="action-title">
                            <Badge tone="positive">具备交易资格</Badge>
                            <span className="muted">
                              {state.mode === "OBSERVE"
                                ? "仅观察，不下单"
                                : "新预测将自动模拟执行"}
                            </span>
                          </div>
                          <div className="plan-stats">
                            <div>
                              <label>建议仓位</label>
                              <strong className="mono">
                                ${money(plan.position_notional)}
                              </strong>
                            </div>
                            <div>
                              <label>有效杠杆</label>
                              <strong className="mono">
                                {money(plan.effective_leverage)}x
                              </strong>
                            </div>
                          </div>
                          <div className="key-values">
                            <div>
                              <span>预期净收益</span>
                              <b className="positive mono">
                                +${money(plan.expected_net_pnl)}
                              </b>
                            </div>
                            <div>
                              <span>计划最大损失</span>
                              <b className="negative mono">
                                −${money(plan.planned_max_loss)}
                              </b>
                            </div>
                            <div>
                              <span>TP / SL</span>
                              <b className="mono">
                                {money(plan.tp)} / {money(plan.sl)}
                              </b>
                            </div>
                            <div>
                              <span>最长持有</span>
                              <b>{plan.horizon}s</b>
                            </div>
                            <div>
                              <span>盈亏平衡概率</span>
                              <b>{pct(plan.break_even_probability)}</b>
                            </div>
                          </div>
                        </>
                      ) : (
                        <div className="no-plan">
                          <div className="shield-circle">
                            <Icon name="shield" size={28} />
                          </div>
                          <h3>等待条件成熟</h3>
                          <p>
                            {q
                              ? `${q.reasons.length} 项检查尚未通过。观察仍在继续，暂不生成可执行计划。`
                              : "行情与预测准备完成后，系统将自动评估交易资格。"}
                          </p>
                          <span>保持空仓，也是一种决策。</span>
                        </div>
                      )}
                      <div className="account-mini">
                        <span>模拟账户净值</span>
                        <b className="mono">${money(state.account.equity)}</b>
                        <span>今日已实现</span>
                        <b
                          className={`mono ${state.account.daily_pnl >= 0 ? "positive" : "negative"}`}
                        >
                          {signed(state.account.daily_pnl)}
                        </b>
                      </div>
                    </section>
                  </div>
                  {p && (
                    <section className="panel tracking">
                      <div className="tracking-title">
                        <span className="status-dot" />
                        <h2>预测实时跟踪</h2>
                        <span className="mono muted">
                          {Math.min(
                            p.horizon,
                            Math.floor((state.now - p.ts) / 1000),
                          )}{" "}
                          / {p.horizon}s
                        </span>
                      </div>
                      <div className="progress-track">
                        <i
                          style={{
                            width: `${Math.min(100, (state.now - p.ts) / (p.horizon * 10))}%`,
                          }}
                        />
                      </div>
                      <div className="tracking-metrics">
                        <div>
                          <label>起始价格</label>
                          <b className="mono">{money(p.start_price)}</b>
                        </div>
                        <div>
                          <label>当前价格</label>
                          <b className="mono">{money(p.current_price)}</b>
                        </div>
                        <div>
                          <label>方向内变动</label>
                          <b className="mono">
                            {signed(
                              (p.current_price / p.start_price - 1) *
                                (p.direction === "LONG" ? 100 : -100),
                              "%",
                            )}
                          </b>
                        </div>
                        <div>
                          <label>距目标</label>
                          <b className="mono">
                            {pct(
                              Math.max(
                                0,
                                p.target -
                                  (p.current_price / p.start_price - 1) *
                                    (p.direction === "LONG" ? 1 : -1),
                              ),
                            )}
                          </b>
                        </div>
                        <div>
                          <label>距止损</label>
                          <b className="mono">
                            {pct(
                              Math.max(
                                0,
                                p.stop +
                                  (p.current_price / p.start_price - 1) *
                                    (p.direction === "LONG" ? 1 : -1),
                              ),
                            )}
                          </b>
                        </div>
                        <div>
                          <label>剩余时间</label>
                          <b className="mono">
                            {Math.max(
                              0,
                              p.horizon - Math.floor((state.now - p.ts) / 1000),
                            )}
                            s
                          </b>
                        </div>
                      </div>
                    </section>
                  )}
                  {position && (
                    <section className="panel position-panel">
                      <div className="panel-heading">
                        <h2>模拟持仓 · {symbol}</h2>
                        <Badge tone="warning">{position.action}</Badge>
                      </div>
                      <div className="tracking-metrics">
                        <div>
                          <label>当前 / 目标仓位</label>
                          <b className="mono">
                            ${money(position.qty * position.current_price)} / $
                            {money(position.target_notional)}
                          </b>
                        </div>
                        <div>
                          <label>当前 / 目标杠杆</label>
                          <b className="mono">
                            {money(
                              (position.qty * position.current_price) /
                                state.account.equity,
                            )}
                            x /{" "}
                            {money(
                              position.target_notional / state.account.equity,
                            )}
                            x
                          </b>
                        </div>
                        <div>
                          <label>入场价格</label>
                          <b className="mono">{money(position.entry_price)}</b>
                        </div>
                        <button
                          className="outline-button"
                          disabled={busy || !connected}
                          onClick={() => command("flatten", { symbol })}
                        >
                          平仓模拟持仓
                        </button>
                      </div>
                      <p className="muted">{position.reason}</p>
                    </section>
                  )}
                  <section className="panel record-panel">
                    <div className="panel-heading">
                      <div>
                        <span className="section-index">05</span>
                        <h2>最近预测</h2>
                        <span className="caption">TRACK RECORD</span>
                      </div>
                      <button
                        className="text-button"
                        onClick={() => go("history")}
                      >
                        全部记录 <Icon name="chevron" size={13} />
                      </button>
                    </div>
                    <OutcomeTable
                      rows={displayedOutcomes.slice(0, 5)}
                      onSelect={(o) => {
                        setSelected(o);
                        setAnnotation(o.annotation || "");
                      }}
                    />
                  </section>
                </>
              )}
              {(tab === "health" || tab === "history") && (
                <IntegrityPanel state={state} />
              )}
              {tab === "history" && (
                <>
                  <div className="summary-cards">
                    <Summary
                      label="当前可比预测"
                      value={`${h?.samples || 0}`}
                      sub={`${symbol} · 当前模型 / 配置`}
                    />
                    <Summary
                      label="预测 / 实际命中率"
                      value={`${pct(h?.predicted_hit_rate, 1)} / ${pct(h?.actual_hit_rate, 1)}`}
                      sub="仅统计有效 First-hit 结果"
                    />
                    <Summary
                      label="模拟已实现净收益"
                      value={`$${money(state.account.realized_pnl)}`}
                      sub="包含手续费与模拟成交成本"
                    />
                  </div>
                  <section className="panel">
                    <div className="panel-heading">
                      <div className="segmented">
                        <button
                          className={
                            historyKind === "prediction" ? "selected" : ""
                          }
                          onClick={() => setHistoryKind("prediction")}
                        >
                          预测结果
                        </button>
                        <button
                          className={historyKind === "trade" ? "selected" : ""}
                          onClick={() => setHistoryKind("trade")}
                        >
                          模拟交易
                        </button>
                      </div>
                      <div className="button-row">
                        <button className="text-button" onClick={openHistory}>
                          加载历史
                        </button>
                        <button className="outline-button" onClick={exportData}>
                          <Icon name="download" size={15} />
                          导出 JSON
                        </button>
                      </div>
                    </div>
                    {historyKind === "prediction" ? (
                      <OutcomeTable
                        rows={displayedOutcomes}
                        onSelect={(o) => {
                          setSelected(o);
                          setAnnotation(o.annotation || "");
                        }}
                      />
                    ) : (
                      <TradeTable
                        rows={state.trades.filter(
                          (t) => t.position.plan.symbol === symbol,
                        )}
                        onSelect={setTrade}
                      />
                    )}
                  </section>
                </>
              )}
              {tab === "health" && h && (
                <>
                  <div className="summary-cards">
                    <Summary
                      label="Model Trust"
                      value={h.trust}
                      sub={h.reasons.join("；") || "根据近期校准与风险表现评估"}
                    />
                    <Summary
                      label="校准误差 ECE"
                      value={pct(h.calibration_error, 1)}
                      sub={`历史基线 ${pct(h.baseline_error, 1)} · Brier ${money(h.brier, 3)}`}
                    />
                    <Summary
                      label="成本后 EV 偏移"
                      value={`${signed(h.ev_drift * 100, "%")}`}
                      sub={`预期 ${pct(h.expected_edge, 3)} / 观察 ${pct(h.observed_edge, 3)}`}
                    />
                  </div>
                  <div className="health-grid">
                    <section className="panel">
                      <div className="panel-heading">
                        <h2>概率与真实命中率</h2>
                        <Badge>{h.samples} 个可比样本</Badge>
                      </div>
                      <HealthBars health={h} />
                      <p className="panel-note">
                        每桶比较预测概率与自动 First-hit
                        结果。数据失效样本不进入学习数据集。
                      </p>
                    </section>
                    <section className="panel">
                      <div className="panel-heading">
                        <h2>偏移监控</h2>
                        <Badge
                          tone={h.retrain_recommended ? "warning" : "muted"}
                        >
                          {h.retrain_recommended ? "建议重训" : "持续观察"}
                        </Badge>
                      </div>
                      <div className="key-values padded">
                        <div>
                          <span>Prediction Drift</span>
                          <b>{pct(h.actual_hit_rate - h.predicted_hit_rate)}</b>
                        </div>
                        <div>
                          <span>PAPER 执行成本偏差</span>
                          <b>
                            {h.execution_drift === null
                              ? "尚无模拟成交"
                              : pct(h.execution_drift, 3)}
                          </b>
                        </div>
                        <div>
                          <span>PAPER 实际净收益偏差</span>
                          <b>
                            {h.paper_ev_drift == null
                              ? "尚无可比交易"
                              : pct(h.paper_ev_drift, 3)}{" "}
                            · {h.paper_samples ?? 0} 笔
                          </b>
                        </div>
                        <div>
                          <span>Regime / OOD</span>
                          <b
                            className={h.regime_drift ? "warning" : "positive"}
                          >
                            {{
                              UNAVAILABLE: "缺少训练分布",
                              SYNTHETIC_ONLY: "仅合成演示",
                              IN_DISTRIBUTION: "训练分布内",
                              OUT_OF_DISTRIBUTION: "超出训练分布",
                            }[h.distribution_status] || "待评估"}
                          </b>
                        </div>
                        <div>
                          <span>当前市场状态</span>
                          <b>{h.regime}</b>
                        </div>
                        <div>
                          <span>状态内样本 / ECE</span>
                          <b>
                            {h.regime_samples} /{" "}
                            {h.regime_calibration_error === null
                              ? "—"
                              : pct(h.regime_calibration_error)}
                          </b>
                        </div>
                        <div>
                          <span>最大标准化偏差</span>
                          <b>
                            {h.distribution_score === null
                              ? "—"
                              : money(h.distribution_score, 2)}
                          </b>
                        </div>
                        <div>
                          <span>强反向结果比例</span>
                          <b>{pct(h.strong_opposite_rate)}</b>
                        </div>
                        <div>
                          <span>MAE · P95</span>
                          <b>{pct(h.mae_p95)}</b>
                        </div>
                      </div>
                      <p className="panel-note">
                        发现偏移时先降低可信度与风险。模型权重不会随单次结果自动变化。
                      </p>
                    </section>
                  </div>
                  <section className="panel model-panel">
                    <div className="panel-heading">
                      <h2>当前模型</h2>
                      <Badge tone="muted">冻结参数</Badge>
                    </div>
                    <div className="detail-grid">
                      {Object.entries(state.model)
                        .filter(([, v]) => v !== null && typeof v !== "object")
                        .map(([k, v]) => (
                          <div key={k}>
                            <span>{k}</span>
                            <b className="mono">{String(v)}</b>
                          </div>
                        ))}
                    </div>
                    <p className="panel-note">
                      参数保持冻结。候选模型须先通过离线报告与影子验证，再由用户在下方晋升。
                      演示模型只能用于 DEMO 数据空间。
                    </p>
                  </section>
                </>
              )}
              {tab === "health" && (
                <ShadowPanel
                  state={state}
                  disabled={!connected || busy}
                  command={command}
                />
              )}
              {tab === "settings" && draft && (
                <form
                  onSubmit={async (e) => {
                    e.preventDefault();
                    await command("config", draft);
                  }}
                >
                  <section className="panel settings-panel">
                    <div className="panel-heading">
                      <div>
                        <h2>风险信封</h2>
                        <Badge>版本 {state.config_version}</Badge>
                      </div>
                      <span className="muted">
                        预测、模拟与执行共同风险边界
                      </span>
                    </div>
                    <div className="settings-grid">
                      {(
                        [
                          ["equity", "初始账户资金", "USDT", 1],
                          [
                            "max_position_notional",
                            "全账户最大名义仓位",
                            "USDT",
                            1,
                          ],
                          [
                            "max_effective_leverage",
                            "最大有效杠杆",
                            "x · 最大 3",
                            1,
                          ],
                          ["risk_per_trade", "单笔风险预算", "%", 100],
                          ["max_daily_loss", "最大日亏损", "USDT", 1],
                          ["max_drawdown", "最大回撤", "%", 100],
                          ["min_net_edge", "最低成本后优势", "%", 100],
                          [
                            "auto_min_net_edge",
                            "AUTO 最低成本后优势",
                            "%",
                            100,
                          ],
                          [
                            "auto_max_concurrent_positions",
                            "AUTO 最大并发持仓",
                            "个",
                            1,
                          ],
                          ["reentry_cooldown_seconds", "重新入场冷却", "秒", 1],
                          [
                            "max_auto_entries_per_window",
                            "窗口入场上限",
                            "次",
                            1,
                          ],
                          [
                            "auto_entry_window_seconds",
                            "入场统计窗口",
                            "秒",
                            1,
                          ],
                          [
                            "max_daily_trades",
                            "每日安全次数上限",
                            "次（非目标）",
                            1,
                          ],
                          ["max_slippage", "最大单边滑点", "%", 100],
                          [
                            "min_liquidation_buffer",
                            "最低清算缓冲估计",
                            "%",
                            100,
                          ],
                          ["target", "预测目标幅度", "%", 100],
                          ["stop", "反向屏障幅度", "%", 100],
                          ["min_probability", "最低命中概率", "%", 100],
                          ["fee_bps", "单边手续费", "bps", 1],
                          ["slippage_bps", "单边预期滑点", "bps", 1],
                          ["min_samples", "最低可比样本数", "个 · 至少 20", 1],
                        ] as [keyof Config, string, string, number][]
                      ).map(([key, label, unit, mult]) => (
                        <label className="field" key={key}>
                          <span>{label}</span>
                          <div>
                            <input
                              aria-label={label}
                              required
                              type="number"
                              step="any"
                              min="0"
                              value={Number(draft[key]) * mult}
                              onChange={(e) =>
                                setDraft({
                                  ...draft,
                                  [key]: Number(e.target.value) / mult,
                                })
                              }
                            />
                            <small>{unit}</small>
                          </div>
                        </label>
                      ))}
                      <label className="field">
                        <span>AUTO 最低模型可信度</span>
                        <select
                          aria-label="AUTO 最低模型可信度"
                          value={draft.auto_required_model_trust}
                          onChange={(e) =>
                            setDraft({
                              ...draft,
                              auto_required_model_trust: e.target.value,
                            })
                          }
                        >
                          <option value="HIGH">HIGH</option>
                          <option value="NORMAL">NORMAL</option>
                        </select>
                      </label>
                      <label className="field">
                        <span>预测窗口</span>
                        <select
                          value={draft.horizon}
                          onChange={(e) =>
                            setDraft({
                              ...draft,
                              horizon: Number(e.target.value),
                            })
                          }
                        >
                          {[5, 15, 30, 60, 120].map((v) => (
                            <option value={v} key={v}>
                              {v} 秒
                            </option>
                          ))}
                        </select>
                      </label>
                      <label className="field">
                        <span>紧急停止后的持仓处理</span>
                        <select
                          value={draft.kill_behavior}
                          onChange={(e) =>
                            setDraft({
                              ...draft,
                              kill_behavior: e.target.value,
                            })
                          }
                        >
                          <option value="hold">保留持仓，继续保护性退出</option>
                          <option value="flatten">按有效报价平仓</option>
                        </select>
                      </label>
                    </div>
                    <details className="advanced-settings">
                      <summary>高级研究与成本参数</summary>
                      <div className="settings-grid">
                        {(
                          [
                            "alpha_eval_interval_ms",
                            "qualification_eval_interval_ms",
                            "min_net_pnl",
                            "adverse_selection_bps",
                            "impact_bps",
                            "funding_bps",
                            "max_spread_bps",
                            "max_volatility",
                            "max_cross_venue_bps",
                            "max_stale_ms",
                            "min_depth",
                            "depth_participation",
                            "calibration_warning",
                            "calibration_low",
                            "ev_drift_limit",
                            "max_rebalances",
                            "rebalance_interval_secs",
                            "position_deadband",
                            "incremental_ev",
                            "signal_enter",
                            "signal_exit",
                            "signal_confirm_ms",
                            "result_hold_ms",
                            "max_opposite_rate",
                            "max_mae_stop_multiple",
                            "high_trust_samples",
                          ] as (keyof Config)[]
                        ).map((key) => (
                          <label className="field" key={key}>
                            <span>{key}</span>
                            <input
                              required
                              type="number"
                              step="any"
                              min="0"
                              value={Number(draft[key])}
                              onChange={(e) =>
                                setDraft({
                                  ...draft,
                                  [key]: Number(e.target.value),
                                })
                              }
                            />
                          </label>
                        ))}
                      </div>
                      <p className="muted">
                        高级字段采用内部单位：比例使用小数，bps
                        为万分之一，时间字段按名称使用秒或毫秒。incremental_ev
                        为独立信号加仓和重新入场的增量优势门槛。AUTO 排序策略为
                        priority-v1。signal_enter / signal_exit
                        为方向进入与退出评分阈值，前者须更大；signal_confirm_ms
                        为方向确认时间，result_hold_ms 为结果最短保留时间。
                      </p>
                    </details>
                    <div className="settings-footer">
                      <p>
                        更改目标 /
                        窗口可能使现有校准模型失配；所有修改均由核心再次校验。
                      </p>
                      <button
                        className="primary-button"
                        disabled={
                          !connected ||
                          busy ||
                          Object.keys(state.positions).length > 0
                        }
                        type="submit"
                      >
                        {busy ? "保存中…" : "保存风险设置"}
                      </button>
                    </div>
                  </section>
                  <section className="panel audit-panel">
                    <div className="panel-heading">
                      <h2>操作审计</h2>
                      <span className="caption">AUDIT LOG</span>
                    </div>
                    {state.audit.length ? (
                      state.audit.slice(0, 15).map((a, i) => (
                        <div className="audit-row" key={i}>
                          <span className="mono muted">{time(a.ts)}</span>
                          <Badge>{a.kind}</Badge>
                          <code>{JSON.stringify(a.payload)}</code>
                        </div>
                      ))
                    ) : (
                      <Empty>尚无配置或控制操作</Empty>
                    )}
                  </section>
                </form>
              )}
            </>
          )}
          <footer className="footer">
            <span>
              <span className="status-dot" />
              MICRO ALPHA <b>PERSONAL v0.4</b>
            </span>
            <span>
              Deterministic core · AI OFF ·{" "}
              {state?.source === "live" ? "真实公共行情" : "合成行情"} ·
              真实下单未开放
            </span>
          </footer>
        </main>
      </div>
      {toast && (
        <div className="toast" role="status">
          <Icon name="pulse" size={17} />
          {toast}
          <button aria-label="关闭通知" onClick={() => setToast("")}>
            <Icon name="close" size={15} />
          </button>
        </div>
      )}
      {selected && (
        <div className="modal-backdrop" onClick={() => setSelected(null)}>
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label="预测结果详情"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="panel-heading">
              <h2>预测结果 · {selected.prediction.symbol}</h2>
              <button
                autoFocus
                aria-label="关闭详情"
                onClick={() => setSelected(null)}
              >
                <Icon name="close" />
              </button>
            </div>
            <Badge
              tone={
                selected.outcome_type === "TARGET_HIT" ? "positive" : "warning"
              }
            >
              {outcomes[selected.outcome_type]}
            </Badge>
            <p className="muted mono">{selected.prediction.prediction_id}</p>
            <div className="detail-grid">
              {[
                ["方向", selected.prediction.direction],
                [
                  "预测命中概率",
                  pct(
                    (selected.prediction.calibrated_probabilities ||
                      selected.prediction.raw_probabilities)[
                      selected.prediction.direction === "LONG" ? 0 : 1
                    ],
                  ),
                ],
                ["首次结算方向收益", pct(selected.final_move)],
                [
                  "到期方向收益",
                  selected.horizon_observation?.status === "OBSERVED"
                    ? pct(selected.horizon_observation.final_move_at_timeout)
                    : selected.horizon_observation?.status ===
                        "DATA_INVALIDATED"
                      ? "数据不足"
                      : "等待到期观察",
                ],
                [
                  "关闭原因",
                  selected.prediction.episode?.close_reason ||
                    selected.outcome_type,
                ],
                ["MFE / MAE", `${pct(selected.mfe)} / ${pct(selected.mae)}`],
                ["观察净优势", pct(selected.observed_net_edge)],
                ["误差分类", selected.prediction_error],
                [
                  "进入学习集",
                  selected.learning_eligible ? "是（来源隔离）" : "否",
                ],
                ["模型版本", selected.prediction.model_version],
              ].map(([k, v]) => (
                <div key={k}>
                  <span>{k}</span>
                  <b>{v}</b>
                </div>
              ))}
            </div>
            <label className="field">
              <span>人工上下文注释（不会覆盖自动结果）</span>
              <textarea
                maxLength={500}
                value={annotation}
                onChange={(e) => setAnnotation(e.target.value)}
                placeholder="例如：市场整体反转、异常波动…"
              />
            </label>
            <button
              className="primary-button"
              disabled={busy || !connected}
              onClick={async () => {
                if (
                  await command("annotate", {
                    prediction_id: selected.prediction.prediction_id,
                    annotation,
                  })
                )
                  setSelected({ ...selected, annotation });
              }}
            >
              保存注释
            </button>
          </section>
        </div>
      )}
      {trade && (
        <div className="modal-backdrop" onClick={() => setTrade(null)}>
          <section
            className="modal wide"
            role="dialog"
            aria-modal="true"
            aria-label="模拟成交详情"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="panel-heading">
              <h2>模拟成交审计</h2>
              <button
                autoFocus
                aria-label="关闭详情"
                onClick={() => setTrade(null)}
              >
                <Icon name="close" />
              </button>
            </div>
            <p className="muted mono">{trade.position.plan.prediction_id}</p>
            <div className="summary-cards">
              <Summary
                label="净收益"
                value={`$${money(trade.net_pnl)}`}
                sub={trade.exit_reason}
              />
              <Summary
                label="预期 / 实际成本"
                value={`$${money(trade.expected_cost)} / $${money(trade.realized_cost)}`}
                sub="模拟账本，不代表真实交易所成交"
              />
            </div>
            <div className="table-scroll">
              <table>
                <thead>
                  <tr>
                    <th>时间</th>
                    <th>方向</th>
                    <th>数量</th>
                    <th>预期 / 成交价</th>
                    <th>手续费</th>
                  </tr>
                </thead>
                <tbody>
                  {trade.position.executions.map((e) => (
                    <tr key={e.order_id}>
                      <td>{time(e.fill_ts)}</td>
                      <td>{e.side}</td>
                      <td>{money(e.qty, 6)}</td>
                      <td>
                        {money(e.expected_price)} / {money(e.fill_price)}
                      </td>
                      <td>${money(e.fee, 4)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}
function Summary({
  label,
  value,
  sub,
}: {
  label: string;
  value: string;
  sub: string;
}) {
  return (
    <div className="summary-card">
      <label>{label}</label>
      <strong className="mono">{value}</strong>
      <p>{sub}</p>
    </div>
  );
}
function OutcomeTable({
  rows,
  onSelect,
}: {
  rows: Outcome[];
  onSelect: (o: Outcome) => void;
}) {
  return rows.length ? (
    <div className="table-scroll">
      <table>
        <thead>
          <tr>
            <th>时间 / 标的</th>
            <th>判断</th>
            <th>预测概率</th>
            <th>实际结果</th>
            <th>方向收益</th>
            <th>MFE / MAE</th>
            <th>详情</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((o) => (
            <tr key={o.prediction.prediction_id}>
              <td>
                <b>{o.prediction.symbol}</b>
                <small>{time(o.prediction.ts)}</small>
              </td>
              <td>
                <span
                  className={
                    o.prediction.direction === "LONG" ? "positive" : "negative"
                  }
                >
                  {o.prediction.direction}
                </span>
                <small>{o.prediction.horizon}s</small>
              </td>
              <td className="mono">
                {pct(
                  (o.prediction.calibrated_probabilities ||
                    o.prediction.raw_probabilities)[
                    o.prediction.direction === "LONG" ? 0 : 1
                  ],
                  1,
                )}
                <small>
                  {o.prediction.calibrated_probabilities ? "校准" : "原始"}
                </small>
              </td>
              <td>
                <Badge
                  tone={
                    o.outcome_type === "TARGET_HIT"
                      ? "positive"
                      : o.outcome_type.endsWith("INVALIDATED")
                        ? "muted"
                        : "warning"
                  }
                >
                  {outcomes[o.outcome_type] || o.outcome_type}
                </Badge>
              </td>
              <td
                className={`mono ${o.final_move >= 0 ? "positive" : "negative"}`}
              >
                {signed(o.final_move * 100, "%")}
              </td>
              <td className="mono muted">
                {pct(o.mfe)} / {pct(o.mae)}
              </td>
              <td>
                <button
                  aria-label={`查看预测 ${o.prediction.prediction_id}`}
                  onClick={() => onSelect(o)}
                >
                  <Icon name="chevron" size={15} />
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  ) : (
    <Empty>尚无已完成预测。系统会自动跟踪屏障触达或超时，无需手动标记。</Empty>
  );
}
function TradeTable({
  rows,
  onSelect,
}: {
  rows: Trade[];
  onSelect: (t: Trade) => void;
}) {
  return rows.length ? (
    <div className="table-scroll">
      <table>
        <thead>
          <tr>
            <th>标的 / 时间</th>
            <th>方向</th>
            <th>投入名义价值</th>
            <th>净收益</th>
            <th>预期 / 实际成本</th>
            <th>退出原因</th>
            <th>成交</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((t) => (
            <tr key={t.position.plan.plan_id}>
              <td>
                {t.position.plan.symbol}
                <small>{time(t.exit_ts)}</small>
              </td>
              <td>{t.position.plan.direction}</td>
              <td>${money(t.position.cost_basis)}</td>
              <td className={t.net_pnl >= 0 ? "positive" : "negative"}>
                ${money(t.net_pnl)}
              </td>
              <td>
                ${money(t.expected_cost)} / ${money(t.realized_cost)}
              </td>
              <td>{t.exit_reason}</td>
              <td>
                <button onClick={() => onSelect(t)} aria-label="查看成交">
                  <Icon name="chevron" size={15} />
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  ) : (
    <Empty>尚无模拟成交。PAPER 只在新预测通过全部资格门槛时入场。</Empty>
  );
}
