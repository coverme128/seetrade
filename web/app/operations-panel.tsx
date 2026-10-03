"use client";
import { useState } from "react";
import type { State } from "./types";
export default function OperationsPanel({
  state,
  symbol,
  busy,
  command,
}: {
  state: State;
  symbol: string;
  busy: boolean;
  command: (name: string, body: unknown) => Promise<unknown>;
}) {
  const [model, setModel] = useState("score");
  const [interval, setInterval] = useState("0");
  const [brief, setBrief] = useState("");
  const [loading, setLoading] = useState(false);
  const [triggerDraft, setTriggerDraft] = useState("");
  const [triggerError, setTriggerError] = useState("");
  const live = state.live;
  const position = state.live_positions?.[symbol];
  const number = (n: number | null | undefined, digits = 2) =>
    n == null ? "—" : n.toFixed(digits);
  const prediction = state.symbols[symbol]?.active;
  const pending = live
    ? Object.values(live.orders)
        .sort((a, b) => b.created_ts - a.created_ts)
        .slice(0, 12)
    : [];
  const job = state.research;
  return (
    <section className="panel padded" data-testid="operations-panel">
      <div className="panel-title">
        <h2>执行与研究</h2>
        <span>
          {live?.configured
            ? `${live.venue} · ${live.environment === "live" ? "真实资金" : "交易所模拟账户"}`
            : "未配置执行账户"}
        </span>
      </div>
      {state.trigger_config && (
        <details className="raw-metrics" data-testid="trigger-diagnostics">
          <summary>运行触发诊断与配置</summary>
          <p>
            {symbol} · {state.symbols[symbol]?.runtime_state ?? "IDLE"} ·
            完整评估 {state.symbols[symbol]?.full_evaluations ?? 0} 次 ·
            触发配置版本
            {state.trigger_config_version}
          </p>
          <p className="muted">
            触发只表示值得评估，不代表买卖信号。阈值是待回放验证的工程默认值；
            原有资格、准入和风险门槛继续生效。修改配置会重新积累当前版本样本。
          </p>
          <pre className="research-result">
            {JSON.stringify(
              {
                origin: state.symbols[symbol]?.trigger_origin,
                pulse: state.symbols[symbol]?.pulse,
                funnel: state.trigger_analytics,
                full_evaluation_latency: state.telemetry?.full_alpha_evaluation,
              },
              null,
              2,
            )}
          </pre>
          <button
            disabled={busy}
            onClick={() =>
              setTriggerDraft(JSON.stringify(state.trigger_config, null, 2))
            }
          >
            载入当前触发配置
          </button>
          {triggerDraft && (
            <>
              <textarea
                aria-label="触发配置 JSON"
                rows={12}
                value={triggerDraft}
                onChange={(event) => setTriggerDraft(event.target.value)}
              />
              <button
                disabled={busy || state.mode !== "OBSERVE"}
                onClick={async () => {
                  try {
                    const value: unknown = JSON.parse(triggerDraft);
                    setTriggerError("");
                    await command("trigger_config", value);
                  } catch {
                    setTriggerError(
                      "配置必须为有效 JSON；服务端还会检查范围和持仓状态。",
                    );
                  }
                }}
              >
                保存触发配置（仅 OBSERVE 空仓）
              </button>
              {triggerError && <p className="warning">{triggerError}</p>}
            </>
          )}
        </details>
      )}
      <p className="muted">
        {state.live_gate_reason ||
          "账户与执行通道已就绪；每笔订单仍须通过当前预测和风险检查。"}
      </p>
      {live?.configured && (
        <>
          <p className="muted">
            执行统计：{state.execution_metrics?.orders ?? 0} 个意图 · 拒绝{" "}
            {state.execution_metrics?.rejection_rate == null
              ? "—"
              : `${(state.execution_metrics.rejection_rate * 100).toFixed(1)}%`}{" "}
            · 待核实 {state.execution_metrics?.unknown ?? 0} · 确认 P95{" "}
            {state.execution_metrics?.ack_p95_ms ?? "—"} ms
          </p>
          <div className="key-values">
            <div>
              <span>独立名义金额上限</span>
              <b>{live.cap.toFixed(2)} USDT</b>
            </div>
            <div>
              <span>账户权益 / 可用资金</span>
              <b>
                {live.account?.equity.toFixed(2) ?? "—"} /{" "}
                {live.account?.available.toFixed(2) ?? "—"}
              </b>
            </div>
          </div>
          {position && (
            <div data-testid="live-position-risk">
              <h3>{symbol} 实盘持仓风险</h3>
              <div className="key-values">
                <div>
                  <span>成交账本 / 账户数量</span>
                  <b>
                    {number(position.quantity, 6)} /{" "}
                    {number(position.account_quantity, 6)}
                  </b>
                </div>
                <div>
                  <span>当前 / 已批准目标名义金额</span>
                  <b>
                    {number(position.current_notional)} /{" "}
                    {number(position.target_notional)} USDT
                  </b>
                </div>
                <div>
                  <span>当前 / 目标有效杠杆</span>
                  <b>
                    {number(position.effective_leverage, 4)} /{" "}
                    {number(position.target_leverage, 4)} ×
                  </b>
                </div>
                <div>
                  <span>账户均价 / 标记价</span>
                  <b>
                    {number(position.entry_price)} /{" "}
                    {number(position.mark_price)}
                  </b>
                </div>
                <div>
                  <span>未实现盈亏 / 强平距离</span>
                  <b>
                    {number(position.unrealized_pnl)} USDT /{" "}
                    {position.liquidation_buffer == null
                      ? "—"
                      : `${(position.liquidation_buffer * 100).toFixed(2)}%`}
                  </b>
                </div>
                <div>
                  <span>止损 / 止盈</span>
                  <b>
                    {number(position.sl)} / {number(position.tp)}
                  </b>
                </div>
              </div>
              <p>
                动作：{position.action} · {position.reason || "等待当前计划"}
              </p>
              <p className="muted">
                账户快照 {position.account_age_ms ?? "—"} ms ·{" "}
                {position.reconciled ? "数量一致" : "数量待对账，禁止新增风险"}{" "}
                · 待处理：
                {position.pending
                  .map((o) => `${o.action} ${o.status}`)
                  .join(" / ") || "无"}
              </p>
              <p className="muted">
                实盘反馈 {position.feedback.status} · 可比样本{" "}
                {position.feedback.samples} · 排除{" "}
                {position.feedback.excluded_samples} · 成本偏差{" "}
                {position.feedback.execution_drift == null
                  ? "—"
                  : `${(position.feedback.execution_drift * 10000).toFixed(2)} bps`}{" "}
                · EV 偏差{" "}
                {position.feedback.ev_drift == null
                  ? "—"
                  : `${(position.feedback.ev_drift * 10000).toFixed(2)} bps`}
              </p>
            </div>
          )}
          <div className="workspace-bar">
            <button
              disabled={
                busy ||
                state.mode !== "CONFIRM" ||
                !prediction ||
                !state.live_enabled ||
                state.symbols[symbol].qualification?.status !== "QUALIFIED"
              }
              onClick={() =>
                command("execute", {
                  symbol,
                  prediction_id: prediction?.prediction_id,
                })
              }
            >
              执行当前计划
            </button>
            <button
              disabled={busy || !prediction}
              onClick={() =>
                command("skip", { prediction_id: prediction?.prediction_id })
              }
            >
              跳过本次预测
            </button>
            <button
              disabled={
                busy ||
                !(
                  position?.quantity ??
                  live.account?.positions[symbol]?.quantity
                )
              }
              onClick={() => command("live_flatten", { symbol })}
            >
              减仓平掉 {symbol}
            </button>
          </div>
          {pending.length > 0 && (
            <div className="table-scroll">
              <table>
                <thead>
                  <tr>
                    <th>订单 / 动作</th>
                    <th>状态</th>
                    <th>成交量 / 均价</th>
                    <th>手续费</th>
                    <th>确认延迟</th>
                  </tr>
                </thead>
                <tbody>
                  {pending.map((o) => (
                    <tr key={o.client_id}>
                      <td>
                        {o.plan.symbol} · {o.action}
                        <small>{o.client_id}</small>
                      </td>
                      <td title={o.reason}>{o.status}</td>
                      <td>
                        {o.filled_qty.toFixed(6)} / {o.average_price.toFixed(2)}
                      </td>
                      <td>{o.fee.toFixed(4)}</td>
                      <td>
                        {o.ack_ts && o.send_ts
                          ? `${Math.max(0, o.ack_ts - o.send_ts)} ms`
                          : "—"}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </>
      )}
      {live?.trades?.length > 0 && (
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>已完成交易</th>
                <th>预期 / 实际净收益</th>
                <th>手续费 / 资金费收入</th>
              </tr>
            </thead>
            <tbody>
              {live.trades
                .slice(-10)
                .reverse()
                .map((t) => (
                  <tr key={t.parent_id}>
                    <td>{t.symbol}</td>
                    <td>
                      {t.expected_net_pnl.toFixed(4)} / {t.net_pnl.toFixed(4)}
                    </td>
                    <td>
                      {t.fees.toFixed(4)} / {t.funding_income.toFixed(4)}
                    </td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>
      )}
      <div className="panel-title">
        <h3>受控重训</h3>
        <span>{job?.status || "IDLE"}</span>
      </div>
      <div className="workspace-bar">
        <label>
          候选模型{" "}
          <select
            aria-label="候选模型"
            value={model}
            onChange={(e) => setModel(e.target.value)}
          >
            <option value="score">确定性基线</option>
            <option value="logistic">Logistic</option>
            <option value="gbt">梯度提升树</option>
          </select>
        </label>
        <button
          disabled={busy || ["QUEUED", "RUNNING"].includes(job?.status || "")}
          onClick={() => command("research_run", { model })}
        >
          立即运行研究
        </button>
        <label>
          定时周期{" "}
          <select
            aria-label="定时重训周期"
            value={interval}
            onChange={(e) => setInterval(e.target.value)}
          >
            <option value="0">关闭</option>
            <option value="3600">每小时</option>
            <option value="86400">每天</option>
            <option value="604800">每周</option>
          </select>
        </label>
        <button
          disabled={busy || ["QUEUED", "RUNNING"].includes(job?.status || "")}
          onClick={() =>
            command("research_schedule", {
              model,
              interval_secs: Number(interval),
            })
          }
        >
          保存计划
        </button>
      </div>
      <p className="muted">
        当前计划：
        {job?.interval_secs
          ? `每 ${job.interval_secs / 3600} 小时 · ${job.model}`
          : "关闭"}
        。训练、校准与测试按时间隔离；候选模型仍须通过影子验证和人工晋升。
      </p>
      {job?.output && (
        <p>
          输出目录：<code>{job.output}</code>
        </p>
      )}
      {job?.result != null && (
        <pre className="research-result">
          {JSON.stringify(job.result, null, 2)}
        </pre>
      )}
      <p className="muted">
        AI 辅助：{state.ai_mode ?? "OFF"} · 仅解释证据，关闭时仍可查看量化摘要。
      </p>
      <button
        disabled={loading}
        onClick={async () => {
          setLoading(true);
          try {
            const r = await fetch("/api/assist", {
              method: "POST",
              headers: {
                "Content-Type": "application/json",
                "X-Micro-Alpha": "local",
              },
              body: JSON.stringify({ symbol }),
            });
            const v = await r.json();
            setBrief(v.text || v.error);
          } catch {
            setBrief("解释服务暂不可用");
          } finally {
            setLoading(false);
          }
        }}
      >
        {loading ? "正在生成解释…" : "解释当前信号与模型状态"}
      </button>
      {brief && <p className="assist-text">{brief}</p>}
    </section>
  );
}
