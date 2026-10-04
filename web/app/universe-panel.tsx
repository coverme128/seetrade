"use client";
import { useState } from "react";
import type { State } from "./types";
import { currentOpportunity } from "./opportunity";
import { OpportunityBadge } from "./opportunity-panel";
import { ShadowPanel } from "./shadow-panel";
const num = (v: unknown, d = 2) =>
  typeof v === "number"
    ? v.toLocaleString("en-US", { maximumFractionDigits: d })
    : "未知";
const time = (t: number | undefined) =>
  t ? new Date(t).toLocaleString("zh-CN", { hour12: false }) : "尚未安排";
const states: Record<string, string> = {
  DISCOVERED: "已发现",
  COLLECTING: "采集中",
  CALIBRATING: "待校准",
  OBSERVE: "观察",
  ELIGIBLE: "已准入",
  DEGRADED: "降级",
  SUSPENDED: "暂停",
  REJECTED: "拒绝",
  RETIRED: "已退市",
};
type Props = {
  state: State;
  busy: boolean;
  command: (name: string, payload: unknown) => Promise<boolean | undefined>;
};
export function PortfolioPanel({ state }: { state: State }) {
  const p = state.portfolio_exposure;
  const u = state.universe;
  if (!p || !u) return null;
  return (
    <section className="panel universe-panel">
      <div className="panel-header">
        <h2>组合敞口</h2>
        <span>
          {u.portfolio.live_enabled ? "实盘组合门禁开启" : "实盘组合门禁关闭"} ·{" "}
          {u.portfolio.enforce_paper ? "模拟执行门禁" : "模拟影子检查"}
        </span>
      </div>
      <div className="universe-stats">
        {Object.entries({
          总名义仓位: `$${num(p.gross_notional)}`,
          "多 / 空": `$${num(p.long_notional)} / $${num(p.short_notional)}`,
          有效杠杆: `${num(p.effective_leverage)}×`,
          风险占用: `$${num(p.risk_used)}`,
          "占用币种（含待成交）": num(p.positions),
          执行场所: String(p.venue),
        }).map(([k, v]) => (
          <div key={k}>
            <small>{k}</small>
            <strong>{v}</strong>
          </div>
        ))}
      </div>
      {Object.entries(u.portfolio_latest).map(([s, d]) => (
        <p
          key={s}
          className={
            d.decision === "BLOCK"
              ? "negative"
              : d.decision === "REDUCE"
                ? "warning"
                : "muted"
          }
        >
          {s} · {d.decision} · ${num(d.requested_notional)} → $
          {num(d.approved_notional)} {d.reason_codes.join(" / ")}{" "}
          {!d.enforced && "（影子）"}
        </p>
      ))}
    </section>
  );
}
export function UniverseSummary({ state }: { state: State }) {
  const u = state.universe;
  if (!u) return null;
  const rows = Object.values(u.profiles);
  return (
    <section className="panel universe-panel">
      <div className="panel-header">
        <h2>动态币种池</h2>
        <span>
          {u.frozen ? "已冻结" : "事件触发运行"} · 扫描
          {u.config.enabled ? "已开启" : "已关闭"}
        </span>
      </div>
      <div className="universe-stats">
        {[
          ["已发现", rows.length],
          ["采集", rows.filter((p) => p.collecting).length],
          [
            "待校准",
            rows.filter((p) => p.lifecycle_state === "CALIBRATING").length,
          ],
          ["准入", rows.filter((p) => p.lifecycle_state === "ELIGIBLE").length],
          [
            "暂停 / 拒绝",
            rows.filter((p) =>
              ["SUSPENDED", "REJECTED", "RETIRED"].includes(p.lifecycle_state),
            ).length,
          ],
        ].map(([k, v]) => (
          <div key={k}>
            <small>{k}</small>
            <strong>{v}</strong>
          </div>
        ))}
      </div>
    </section>
  );
}
export function ScannerPanel({ state, busy, command }: Props) {
  const [target, setTarget] = useState("");
  const [selected, setSelected] = useState("BTCUSDT");
  const [sort, setSort] = useState("priority");
  const [page, setPage] = useState(0);
  const [filter, setFilter] = useState("");
  const [editor, setEditor] = useState("");
  const [editKind, setEditKind] = useState("asset_config");
  const [error, setError] = useState("");
  const [evidence, setEvidence] = useState("");
  const u = state.universe;
  if (!u)
    return (
      <section className="panel universe-panel">核心尚未迁移到 v1.1。</section>
    );
  const priority: Record<string, number> = {
    HIGH: 3,
    MEDIUM: 2,
    LOW: 1,
    REJECT: 0,
  };
  const rows = Object.values(u.profiles)
    .filter((p) => p.asset_key.includes(filter.toUpperCase()))
    .sort((a, b) =>
      sort === "volume"
        ? (b.observation?.metrics.quote_volume || 0) -
          (a.observation?.metrics.quote_volume || 0)
        : priority[b.qualification?.research_priority || "LOW"] -
            priority[a.qualification?.research_priority || "LOW"] ||
          a.asset_key.localeCompare(b.asset_key),
    );
  const currentPage = Math.min(
    page,
    Math.max(0, Math.ceil(rows.length / 50) - 1),
  );
  const p = u.profiles[selected];
  const queued = busy || !!u.running || u.requested;
  const config = (key: string, value: unknown) =>
    command("scanner_config", { ...u.config, [key]: value });
  return (
    <>
      <UniverseSummary state={state} />
      <section className="panel universe-panel">
        <div className="panel-header">
          <h2>币种扫描器</h2>
          <button
            disabled={queued || u.frozen}
            onClick={() => command("scanner_run", {})}
          >
            {queued ? "扫描任务处理中…" : "立即扫描"}
          </button>
        </div>
        <p className="muted">
          研究优先级只用于决定先采集和研究哪些币种。新增币种默认只允许 OBSERVE。
        </p>
        <div className="universe-controls">
          <label>
            <input
              type="checkbox"
              checked={u.config.enabled}
              disabled={queued || u.frozen}
              onChange={(e) => config("enabled", e.target.checked)}
            />{" "}
            定时扫描
          </label>
          <label>
            周期{" "}
            <select
              aria-label="扫描周期"
              value={u.config.interval_secs}
              disabled={queued}
              onChange={(e) => config("interval_secs", Number(e.target.value))}
            >
              <option value={60}>1 分钟</option>
              <option value={300}>5 分钟</option>
              <option value={900}>15 分钟</option>
            </select>
          </label>
          <label>
            <input
              type="checkbox"
              checked={u.config.dry_run}
              disabled={queued}
              onChange={(e) => config("dry_run", e.target.checked)}
            />{" "}
            只扫描，手动决定采集
          </label>
          <span>
            槽位 {Object.values(u.profiles).filter((p) => p.collecting).length}{" "}
            / {u.config.candidate_l2_slots}
          </span>
        </div>
        <p>
          上次：{time(u.last_run?.completed_at)} ·{" "}
          {u.last_run?.status || "尚未扫描"} · 考察{" "}
          {u.last_run?.considered || 0} 个合约 · 下次：
          {u.config.enabled ? time(u.next_run) : "关闭"}
        </p>
        {Object.entries(u.last_run?.errors || {}).map(([source, message]) => (
          <p className="warning" key={source}>
            {source}：{message}
          </p>
        ))}
        <form
          className="universe-controls"
          onSubmit={(e) => {
            e.preventDefault();
            command("asset_add", { symbol: target });
          }}
        >
          <label>
            手动添加合约{" "}
            <input
              aria-label="手动添加合约"
              placeholder="SOLUSDT"
              value={target}
              onChange={(e) => setTarget(e.target.value)}
            />
          </label>
          <button disabled={queued || !target.trim() || u.frozen}>
            验证并扫描
          </button>
        </form>
        <div className="universe-controls">
          <input
            aria-label="筛选币种"
            placeholder="筛选币种"
            value={filter}
            onChange={(e) => {
              setFilter(e.target.value);
              setPage(0);
            }}
          />
          <label>
            排序{" "}
            <select
              aria-label="扫描器排序"
              value={sort}
              onChange={(e) => setSort(e.target.value)}
            >
              <option value="priority">研究优先级</option>
              <option value="volume">24h 成交额</option>
            </select>
          </label>
        </div>
        <div className="table-scroll">
          <table data-testid="scanner-table">
            <thead>
              <tr>
                {[
                  "币种 / 场所",
                  "生命周期",
                  "研究优先级",
                  "24h 成交额",
                  "价差中位 / P95 bps",
                  "深度 / 滑点 bps",
                  "波动 / 跳变",
                  "数据就绪",
                  "原因 / 操作",
                ].map((x) => (
                  <th key={x}>{x}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.slice(currentPage * 50, (currentPage + 1) * 50).map((a) => {
                const m = a.observation?.metrics;
                const q = a.qualification;
                return (
                  <tr key={a.asset_key}>
                    <td>
                      <button
                        onClick={() => {
                          setSelected(a.asset_key);
                          setEditor("");
                          requestAnimationFrame(() =>
                            document
                              .getElementById("asset-profile")
                              ?.scrollIntoView({
                                behavior: "smooth",
                                block: "start",
                              }),
                          );
                        }}
                      >
                        {a.asset_key}
                      </button>
                      <small>
                        {a.supported_exchanges.join(" / ") || "未确认"}
                      </small>
                    </td>
                    <td>
                      {states[a.lifecycle_state] || a.lifecycle_state}
                      <small>{a.lifecycle_state}</small>
                    </td>
                    <td>
                      <span
                        className={`badge ${q?.research_priority === "HIGH" ? "positive" : q?.research_priority === "REJECT" ? "negative" : "warning"}`}
                      >
                        {q?.research_priority || "待扫描"}
                      </span>
                    </td>
                    <td>${num(m?.quote_volume, 0)}</td>
                    <td>
                      {num(m?.median_spread_bps)} / {num(m?.tail_spread_bps)}
                    </td>
                    <td>
                      ${num(m?.depth, 0)} / {num(m?.slippage_bps)}
                    </td>
                    <td>
                      {num(m?.volatility, 5)} / {num(m?.tail_gap, 5)}
                    </td>
                    <td>
                      {a.clean_since
                        ? `${num((state.now - a.clean_since) / 3600000)}h / ${u.config.minimum_clean_hours}h`
                        : "预热不足"}
                      <small>
                        {num(m?.samples, 0)} 样本 · 重置 {num(m?.resets, 0)}
                      </small>
                    </td>
                    <td>
                      <small>
                        {[
                          ...(q?.blocking_reasons || []),
                          ...(q?.warnings || []),
                        ].join(", ") || "等待评估"}
                      </small>
                      <button
                        disabled={busy || u.frozen}
                        onClick={() =>
                          command("asset_collect", {
                            symbol: a.asset_key,
                            enabled: !a.collecting,
                          })
                        }
                      >
                        {a.collecting ? "停止采集" : "开始采集"}
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </section>
      <div className="universe-controls">
        <span>共 {rows.length} 个币种 · 每页 50 个</span>
        <button
          disabled={currentPage === 0}
          onClick={() => setPage(currentPage - 1)}
        >
          上一页
        </button>
        <span>
          {currentPage + 1} / {Math.max(1, Math.ceil(rows.length / 50))}
        </span>
        <button
          disabled={(currentPage + 1) * 50 >= rows.length}
          onClick={() => setPage(currentPage + 1)}
        >
          下一页
        </button>
      </div>
      {p && (
        <section id="asset-profile" className="panel universe-panel">
          <div className="panel-header">
            <h2>
              {selected} · 币种档案 v{p.profile_version}
            </h2>
            <span>{states[p.lifecycle_state]}</span>
          </div>
          <p>
            权限：{p.mode_permissions.join(" / ")} · 模型{" "}
            {p.model?.model_version || "未校准"} · 触发档案 {p.trigger.id}{" "}
            {p.trigger.calibrated ? "已校准" : "尚未校准"}
          </p>
          <p>
            归一化：{p.feature_normalization_version} · 校准：
            {p.calibration_artifact_id || "未知"} · 成本：
            {p.cost_artifact_id || "默认成本假设，待校准"}
            {" · Trust："}
            {state.symbols[selected]?.health.trust || "未知"}
            {" · 流动性："}
            {p.liquidity_artifact_id || "尚无扫描证据"}
          </p>
          <div className="universe-controls">
            <button
              disabled={busy}
              onClick={() =>
                command("research_run", { symbol: selected, model: "score" })
              }
            >
              研究此币种
            </button>
            <button
              disabled={busy}
              onClick={() =>
                command("asset_trigger_calibrate", { symbol: selected })
              }
            >
              校准当前触发分布
            </button>
            <button
              disabled={busy || p.lifecycle_state !== "OBSERVE"}
              onClick={() => command("asset_review", { symbol: selected })}
            >
              检查晋级 ELIGIBLE
            </button>
            <button
              disabled={busy}
              onClick={() =>
                command("asset_permissions", {
                  symbol: selected,
                  modes: p.mode_permissions.includes("PAPER")
                    ? ["OBSERVE"]
                    : ["OBSERVE", "PAPER"],
                })
              }
            >
              {p.mode_permissions.includes("PAPER")
                ? "撤销模拟权限"
                : "授予模拟权限"}
            </button>
          </div>
          <p className="muted">
            晋级要求持续数据、{u.config.minimum_outcomes}{" "}
            条同版本有效结果、重复扫描通过、币种独立校准与触发参数；实盘权限需要单独配置。
          </p>
          <div className="universe-controls">
            {["CONFIRM", "AUTO"].map((mode) => (
              <label key={mode}>
                <input
                  type="checkbox"
                  checked={p.mode_permissions.includes(mode)}
                  disabled={
                    busy ||
                    p.lifecycle_state !== "ELIGIBLE" ||
                    !u.portfolio.live_enabled
                  }
                  onChange={(e) =>
                    command("asset_permissions", {
                      symbol: selected,
                      modes: e.target.checked
                        ? [...p.mode_permissions, mode]
                        : p.mode_permissions.filter((x) => x !== mode),
                    })
                  }
                />{" "}
                {mode} 权限
              </label>
            ))}
          </div>
          <details>
            <summary>资格检查明细与状态记录</summary>
            {Object.entries(p.qualification?.components || {}).map(([k, v]) => (
              <p key={k}>
                {k}：{v.state} · {JSON.stringify(v.evidence)}
              </p>
            ))}
            {u.transitions
              .filter((t) => t.asset_key === selected)
              .slice(-12)
              .reverse()
              .map((t) => (
                <p key={t.id}>
                  {time(t.created_at)} · {t.from_state} → {t.to_state} ·{" "}
                  {t.reason_code}
                </p>
              ))}
          </details>
          <details>
            <summary>版本化研究参数与组合设置</summary>
            <div className="universe-controls">
              {[
                ["asset_config", "币种成本 / 风险 / H-T-S"],
                ["asset_trigger_config", "币种触发参数"],
                ["portfolio_config", "组合门禁"],
                ["scanner_config", "扫描与预热规则"],
              ].map(([kind, label]) => (
                <button
                  key={kind}
                  onClick={() => {
                    setEditKind(kind);
                    setError("");
                    setEditor(
                      JSON.stringify(
                        kind === "asset_config"
                          ? {
                              ...(p.config || state.config),
                              venue_costs:
                                (p.config || state.config).venue_costs || {},
                            }
                          : kind === "asset_trigger_config"
                            ? p.trigger.config
                            : kind === "portfolio_config"
                              ? u.portfolio
                              : u.config,
                        null,
                        2,
                      ),
                    );
                  }}
                >
                  {label}
                </button>
              ))}
            </div>
            {editor && (
              <>
                <textarea
                  aria-label="版本化参数 JSON"
                  className="universe-editor"
                  value={editor}
                  onChange={(e) => setEditor(e.target.value)}
                />
                {editKind === "asset_trigger_config" && (
                  <input
                    aria-label="触发校准证据"
                    placeholder="校准 / 回放证据路径及版本"
                    value={evidence}
                    onChange={(e) => setEvidence(e.target.value)}
                  />
                )}
                <button
                  disabled={busy}
                  onClick={() => {
                    try {
                      const value = JSON.parse(editor);
                      command(
                        editKind,
                        editKind.startsWith("asset_")
                          ? { symbol: selected, config: value, evidence }
                          : value,
                      );
                    } catch {
                      setError("JSON 格式无效");
                    }
                  }}
                >
                  验证并保存配置
                </button>
                <p className="negative">{error}</p>
              </>
            )}
          </details>
          <p>
            研究状态：{state.research.status} ·{" "}
            {state.research.output || "尚无研究工件"}
          </p>
          <ShadowPanel
            state={state}
            disabled={busy}
            command={(name, payload) =>
              command(
                name,
                name === "shadow_start"
                  ? { ...(payload as object), symbol: selected }
                  : payload,
              )
            }
          />
        </section>
      )}
      <PortfolioPanel state={state} />
      <section className="panel universe-panel">
        <h2>迁移回退</h2>
        <p>
          冻结动态发现和新增采集，恢复 BTC / ETH
          工作集；其他币种已有持仓继续接收行情保护。档案和历史保留。
        </p>
        <button
          disabled={queued}
          onClick={() => command("universe_rollback", { frozen: !u.frozen })}
        >
          {u.frozen ? "解除动态池冻结" : "冻结动态池并切回观察"}
        </button>
      </section>
    </>
  );
}
export function RadarPanel({
  state,
  connected,
  now,
  onSelect,
}: {
  state: State;
  connected: boolean;
  now: number;
  onSelect: (s: string) => void;
}) {
  const [sort, setSort] = useState("quality");
  const [showBlocked, setShowBlocked] = useState(false);
  const all = Object.entries(state.symbols);
  const rows = all
    .filter(([, s]) => !!s.active)
    .map(([symbol, s]) => ({
      symbol,
      s,
      grade: currentOpportunity(state, symbol, connected, now),
    }))
    .filter(
      (r) => showBlocked || !["blocked", "inactive"].includes(r.grade.level),
    )
    .sort((a, b) =>
      sort === "time"
        ? b.s.active!.ts - a.s.active!.ts
        : b.grade.rank - a.grade.rank ||
          (b.s.qualification?.net_edge || 0) -
            (a.s.qualification?.net_edge || 0) ||
          b.s.active!.ts - a.s.active!.ts,
    );
  return (
    <>
      <UniverseSummary state={state} />
      <section className="panel universe-panel">
        <div className="panel-header">
          <h2>机会雷达</h2>
          <span>
            触发 {all.filter(([, s]) => s.runtime_state !== "IDLE").length} ·
            活跃 {all.filter(([, s]) => s.active).length} · 合格{" "}
            {
              all.filter(
                ([, s]) => s.active && s.qualification?.status === "QUALIFIED",
              ).length
            }
          </span>
        </div>
        <p className="muted">
          只展示正式预测中的机会。研究排名、空闲币种和无优势的瞬时脉冲不会生成交易卡片。
        </p>
        <div className="universe-controls">
          <label>
            排序{" "}
            <select
              aria-label="雷达排序"
              value={sort}
              onChange={(e) => setSort(e.target.value)}
            >
              <option value="quality">资格 → 净优势 → 最近更新</option>
              <option value="time">最新预测</option>
            </select>
          </label>
          <label>
            <input
              type="checkbox"
              checked={showBlocked}
              onChange={(e) => setShowBlocked(e.target.checked)}
            />{" "}
            显示受阻 / 降级机会
          </label>
        </div>
        <div className="radar-grid">
          {rows.map(({ symbol, s, grade }) => {
            const p = s.active!;
            const probs =
              p.episode?.latest_calibrated_probabilities ||
              p.calibrated_probabilities;
            const admission = state.admission?.latest?.[symbol];
            const profile = state.universe?.profiles[symbol];
            return (
              <article
                key={p.prediction_id}
                className={`panel radar-card opportunity-${grade.level}`}
                data-testid="radar-card"
              >
                <div className="panel-header">
                  <h3>
                    {symbol} · {p.direction}
                  </h3>
                  <OpportunityBadge opportunity={grade} />
                </div>
                <p>
                  {p.horizon}s · 目标 {(p.target * 100).toFixed(2)}% / 止损{" "}
                  {(p.stop * 100).toFixed(2)}%
                </p>
                <p>
                  校准概率 ↑ / ↓ / 超时：
                  {probs
                    ? probs.map((x) => `${(x * 100).toFixed(1)}%`).join(" / ")
                    : "未校准"}
                </p>
                <p>
                  净优势 {num((s.qualification?.net_edge || 0) * 100, 3)}% ·
                  Trust {s.health.trust}
                </p>
                <p>{grade.reason}</p>
                <ul>
                  <li>订单簿不平衡 {num(s.features?.book_imbalance)}</li>
                  <li>主动买卖失衡 {num(s.features?.aggressive_imbalance)}</li>
                  <li>动量 {num(s.features?.momentum, 5)}</li>
                </ul>
                <p>
                  权限 {profile?.mode_permissions.join(" / ") || "原有配置"} ·
                  AUTO {admission?.decision || "尚未准入"}
                </p>
                <button onClick={() => onSelect(symbol)}>
                  查看预测与执行检查
                </button>
              </article>
            );
          })}
        </div>
        {!rows.length && (
          <p className="empty">暂无活跃交易机会，继续监听市场变化。</p>
        )}
      </section>
      <PortfolioPanel state={state} />
    </>
  );
}
