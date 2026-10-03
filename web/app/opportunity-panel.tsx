"use client";
import type { State } from "./types";
import {
  compareOpportunity,
  currentOpportunity,
  type Opportunity,
} from "./opportunity";

export function OpportunityBadge({
  opportunity,
}: {
  opportunity: Opportunity;
}) {
  return (
    <span
      className={`opportunity-badge opportunity-${opportunity.level}`}
      title={opportunity.reason}
    >
      <span aria-hidden="true" className="opportunity-dot" />
      {opportunity.label}
    </span>
  );
}

export default function OpportunityPanel({
  state,
  symbol,
  connected,
  now,
  onSelect,
}: {
  state: State;
  symbol: string;
  connected: boolean;
  now: number;
  onSelect: (symbol: string) => void;
}) {
  const rows = Object.keys(state.symbols)
    .map((key) => ({
      symbol: key,
      opportunity: currentOpportunity(state, key, connected, now),
    }))
    .sort(
      (a, b) =>
        compareOpportunity(a.opportunity, b.opportunity) ||
        a.symbol.localeCompare(b.symbol),
    );
  const priority = rows
    .filter((r) => r.opportunity.level === "priority")
    .map((r) => r.symbol.replace("USDT", ""));
  return (
    <section
      className="panel opportunity-panel"
      data-testid="live-opportunities"
      aria-label="当前机会等级"
    >
      <div className="opportunity-heading">
        <h2>
          当前机会 <small>等级优先 · BTC / ETH</small>
        </h2>
        <span
          data-testid="opportunity-notice"
          aria-live="polite"
          className={priority.length ? "opportunity-priority" : "muted"}
        >
          {priority.length
            ? `优先关注：${priority.join("、")}`
            : "当前暂无优先机会"}
        </span>
      </div>
      <div className="opportunity-grid">
        {rows.map(({ symbol: key, opportunity: o }) => (
          <button
            type="button"
            key={key}
            data-testid={`opportunity-${key}`}
            data-level={o.level}
            className={`opportunity-tile opportunity-${o.level}`}
            aria-pressed={symbol === key}
            onClick={() => onSelect(key)}
          >
            <span className="opportunity-tile-top">
              <b>{key.replace("USDT", "")} / USDT</b>
              <OpportunityBadge opportunity={o} />
            </span>
            <span>
              {state.symbols[key].active?.direction || "FLAT"} ·{" "}
              {o.netEdge == null
                ? "等待有效交易条件"
                : `成本后优势 +${(o.netEdge * 100).toFixed(3)}%`}
            </span>
            <small>{o.reason}</small>
          </button>
        ))}
      </div>
      <p className="opportunity-legend">
        绿：优先 · 蓝：合格 · 黄：待验证 · 红：未达标 · 灰：暂无 /
        不可用。颜色表示资格与可信度，买卖方向单独显示；优先不等于已获 AUTO
        准入或保证收益。
      </p>
    </section>
  );
}
