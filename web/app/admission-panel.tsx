"use client";
import { useState } from "react";
import type { AdmissionDecision, State } from "./types";
const labels: Record<string, string> = {
  NOT_QUALIFIED: "普通交易资格未通过",
  AUTO_EDGE_THRESHOLD_NOT_MET: "未达到 AUTO 专用净优势门槛",
  MODEL_TRUST_INSUFFICIENT: "模型可信度不足",
  STALE_EVALUATION: "最新评估已过期",
  EPISODE_EXPIRED: "预测已结束或配置已变化",
  EXECUTION_QUALITY_FAIL: "行情或执行通道检查未通过",
  RISK_BUDGET_UNAVAILABLE: "可用风险预算不足",
  BETTER_OPPORTUNITY_ACTIVE: "预算优先分配给更高优先级机会",
  REENTRY_COOLDOWN: "同资产重新入场冷却中",
  FRESH_ALPHA_REQUIRED: "退出后尚未形成独立新信号",
  REENTRY_COST_NOT_COVERED: "新增优势不足以覆盖重新入场成本",
  DAILY_TRADE_CAP: "达到每日安全次数上限",
  ORDER_WINDOW_CAP: "达到窗口内入场次数上限",
  POSITION_ALREADY_ACTIVE: "已有持仓或待执行订单",
  EPISODE_ALREADY_CONSUMED: "本次预测已执行或已跳过",
  AUTO_LIVE_EVIDENCE_REQUIRED: "AUTO 授权或多窗口验证证据未就绪",
};
export function AdmissionCard({
  state,
  symbol,
}: {
  state: State;
  symbol: string;
}) {
  const d = state.admission?.latest[symbol];
  const p = state.symbols[symbol]?.active;
  if (!p) return null;
  const current =
    d?.episode_id === p.prediction_id &&
    d.config_version === state.config_version;
  const fresh =
    current && state.now - d.evaluated_at <= state.config.max_stale_ms;
  const live = state.mode === "AUTO";
  return (
    <div className="padded admission-card" data-testid="admission-card">
      <strong>
        {live ? "AUTO" : "AUTO 影子准入"}: {fresh ? d.decision : "等待最新评估"}
      </strong>
      {fresh && (
        <p title={d.reason_codes.join(", ")}>
          {d.reason_codes.length
            ? labels[d.reason_codes[0]] || d.reason_codes[0]
            : "准入通过，仍须通过发送前风控检查。"}
          {d.reason_codes.length > 0 && (
            <small className="mono">{d.reason_codes[0]}</small>
          )}
        </p>
      )}
      <p className="muted">
        交易资格与资金准入分别判断。
        {live
          ? "ADMITTED 表示准入通过；成交以交易所回报为准。"
          : "影子结果不产生实盘订单。"}
      </p>
      {fresh && (
        <details>
          <summary>准入依据与预算</summary>
          <p className="mono">
            {d.admission_id} · {d.priority_tuple.policy_version}
          </p>
          <p>
            净优势 {(d.priority_tuple.net_edge_after_cost * 100).toFixed(3)}% ·
            信任等级 {d.priority_tuple.model_trust} · 计划风险{" "}
            {d.priority_tuple.risk_consumption.toFixed(2)} USDT
          </p>
          <p>
            可用风险预算 {d.risk_budget_before.available_loss.toFixed(2)} →{" "}
            {d.risk_budget_after_if_admitted.available_loss.toFixed(2)} USDT
          </p>
          <p>并发候选：{d.competing_episode_ids.join(" / ") || "无"}</p>
          <p>{d.reason_codes.map((r) => labels[r] || r).join("；")}</p>
        </details>
      )}
    </div>
  );
}
export function IntegrityPanel({ state }: { state: State }) {
  const [rows, setRows] = useState<AdmissionDecision[]>([]);
  const [status, setStatus] = useState("");
  const c = state.daily_integrity;
  return (
    <section className="panel padded" data-testid="integrity-panel">
      <div className="panel-title">
        <h2>预测与执行完整性</h2>
        <span>当日 UTC · 自动累计</span>
      </div>
      <p>
        实时评估 {c?.realtime_evaluations ?? 0} · Episode 开启{" "}
        {c?.prediction_episodes_opened ?? 0} · 关闭{" "}
        {c?.prediction_episodes_closed ?? 0} · 曾通过资格{" "}
        {c?.qualified_episodes ?? 0}
      </p>
      <p>
        PAPER 完成交易 {c?.paper_trades ?? 0} · 交易所完成交易{" "}
        {c?.live_trades ?? 0}
      </p>
      <p className="muted">
        评估次数不计入预测样本；没有交易的 Episode
        仍自动产生结果。允许全天零交易。
      </p>
      <button
        onClick={async () => {
          try {
            const response = await fetch(
              "/api/history?kind=auto_admission&limit=50",
            );
            if (!response.ok) throw new Error("读取准入记录失败");
            const data = await response.json();
            setRows(data.map((r: { payload: AdmissionDecision }) => r.payload));
            setStatus("已加载最近 50 条以内准入记录");
          } catch (e) {
            setStatus(e instanceof Error ? e.message : "读取失败");
          }
        }}
      >
        加载准入审计
      </button>
      {status && <p role="status">{status}</p>}
      {rows.length > 0 && (
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>Episode / 环境</th>
                <th>准入结果</th>
                <th>主要原因</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((d) => (
                <tr key={d.admission_id}>
                  <td>
                    {d.symbol} · {d.context}
                    <small>{d.episode_id}</small>
                  </td>
                  <td>{d.decision}</td>
                  <td>{d.reason_codes[0] || "PASS"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
