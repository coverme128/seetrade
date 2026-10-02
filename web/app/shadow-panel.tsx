"use client";
import { useState } from "react";
import type { State, ShadowMetrics } from "./types";

const checkNames: Record<string, string> = {
  running: "验证正在运行",
  same_context: "模型与配置保持一致",
  samples: "有效配对样本",
  both_symbols: "BTC / ETH 均有足够样本",
  fresh_evidence: "最近证据未过期",
  data_quality: "数据失效率",
  candidate_metrics: "整体校准、收益与风险",
  window_stability: "三个时间窗口均稳定",
  regime_stability: "市场状态分组均通过",
  champion_comparison: "不劣于当前模型",
  observe_mode: "当前处于 OBSERVE",
  no_positions: "无模拟持仓",
  storage_healthy: "记录存储正常",
  kill_released: "紧急停止已解除",
  market_ready: "行情新鲜、当前状态有校准且在训练分布内",
};
const statuses: Record<string, string> = {
  RUNNING: "正在观察",
  STOPPED: "已停止",
  PAUSED_CONFIG: "配置变化，需重新验证",
  PROMOTED: "已晋升，保持观察",
  ROLLED_BACK: "已回退",
};
const percent = (n: number) => `${(n * 100).toFixed(2)}%`;
const metrics: [keyof ShadowMetrics, string, boolean][] = [
  ["samples", "有效样本", false],
  ["selected", "成本与信号筛选样本", false],
  ["coverage", "校准及分布覆盖率", true],
  ["ece", "校准误差 ECE", true],
  ["brier", "Brier 误差", false],
  ["expected_net_edge", "筛选样本预期净优势", true],
  ["observed_net_edge", "筛选样本实际净优势", true],
  ["drawdown", "累计单位名义价值回撤", true],
  ["tail_p05", "收益尾部 P05", true],
  ["strong_opposite_rate", "强反向结果比例", true],
];
export function ShadowPanel({
  state,
  disabled,
  command,
}: {
  state: State;
  disabled: boolean;
  command: (name: string, payload: unknown) => Promise<boolean | undefined>;
}) {
  const [artifact, setArtifact] = useState("");
  const [report, setReport] = useState("");
  const [policy, setPolicy] = useState("");
  const [error, setError] = useState("");
  const run = state.shadow;
  const running = run?.status === "RUNNING";
  const load = async (file: File | undefined, setter: (v: string) => void) => {
    setter("");
    setError("");
    if (!file) return;
    if (file.size > 64 * 1024) {
      setError("每个文件最多 64 KB，请使用研究工具导出的 JSON。");
      return;
    }
    try {
      setter(await file.text());
    } catch {
      setError("文件读取失败，请重新选择。");
    }
  };
  const exportEvidence = () => {
    const blob = new Blob(
      [JSON.stringify({ source: state.source, shadow: run }, null, 2)],
      { type: "application/json" },
    );
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${run?.run_id || "shadow"}-comparison.json`;
    a.click();
    URL.revokeObjectURL(url);
  };
  return (
    <section
      className="panel model-panel shadow-panel"
      aria-label="影子模型验证"
    >
      <div className="panel-heading">
        <h2>影子模型与晋升</h2>
        <span className="badge muted">
          {run ? statuses[run.status] || run.status : "尚未载入候选模型"}
        </span>
      </div>
      <p className="panel-note">
        候选模型与当前模型在相同时间、方向和价格屏障下比较。影子模型不会下单；晋升后仍保持
        OBSERVE，模拟入场继续受全部风险门槛约束。
      </p>
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setError("");
          try {
            const parsedReport = JSON.parse(report);
            const parsedPolicy = JSON.parse(
              policy || JSON.stringify(state.shadow_policy_defaults),
            );
            await command("shadow_start", {
              artifact,
              report: parsedReport,
              policy: parsedPolicy,
            });
          } catch {
            setError("报告或阈值不是有效 JSON，请检查文件与输入。");
          }
        }}
      >
        <div className="settings-grid">
          <label className="field">
            <span>候选模型 challenger.json</span>
            <input
              aria-label="候选模型文件"
              type="file"
              accept=".json,application/json"
              disabled={disabled || running}
              onChange={(e) => void load(e.target.files?.[0], setArtifact)}
            />
          </label>
          <label className="field">
            <span>离线验证 report.json</span>
            <input
              aria-label="离线报告文件"
              type="file"
              accept=".json,application/json"
              disabled={disabled || running}
              onChange={(e) => void load(e.target.files?.[0], setReport)}
            />
          </label>
        </div>
        <details className="advanced-settings">
          <summary>晋升阈值（仅新验证生效）</summary>
          <p className="muted">
            阈值随本次验证冻结。比例用小数，max_evidence_age_ms
            为证据最大年龄（毫秒）。运行中修改交易配置会暂停影子验证。
          </p>
          <textarea
            aria-label="影子验证阈值 JSON"
            rows={15}
            value={
              policy || JSON.stringify(state.shadow_policy_defaults, null, 2)
            }
            disabled={disabled || running}
            onChange={(e) => setPolicy(e.target.value)}
          />
        </details>
        {error && (
          <p role="alert" className="negative">
            {error}
          </p>
        )}
        <div className="settings-footer">
          <button
            type="submit"
            className="primary-button"
            disabled={disabled || running || !artifact || !report}
          >
            载入并开始影子验证
          </button>
          {running && (
            <button
              type="button"
              className="text-button"
              disabled={disabled}
              onClick={() => command("shadow_stop", { run_id: run.run_id })}
            >
              停止影子验证
            </button>
          )}
        </div>
      </form>
      {run && (
        <>
          <div className="detail-grid">
            <div>
              <span>候选模型</span>
              <b className="mono">{run.candidate_version}</b>
            </div>
            <div>
              <span>对比模型</span>
              <b className="mono">{run.champion_version}</b>
            </div>
            <div>
              <span>独立验证编号</span>
              <b className="mono">{run.run_id}</b>
            </div>
            <div>
              <span>尚待结果 / 已记录</span>
              <b>
                {run.comparison.pending} / {run.comparison.retained}
              </b>
            </div>
          </div>
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>同一组预测时点</th>
                  <th>当前模型</th>
                  <th>候选模型</th>
                </tr>
              </thead>
              <tbody>
                {metrics.map(([key, label, ratio]) => (
                  <tr key={key}>
                    <td>{label}</td>
                    {[run.comparison.champion, run.comparison.candidate].map(
                      (m, i) => (
                        <td key={i} className="mono">
                          {ratio ? percent(m[key]) : Number(m[key].toFixed(4))}
                        </td>
                      ),
                    )}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="panel-note">
            净优势使用实际价格路径扣除配置成本，回撤按单位名义金额累计，均不是模拟成交收益。
            {!run.comparison.champion_economics_comparable &&
              "当前模型缺少可比的已校准筛选样本：比较其预测误差，同时要求候选通过绝对收益与风险上限。"}
          </p>
          <div className="detail-grid" data-testid="shadow-gates">
            {Object.entries({
              ...run.comparison.checks,
              ...run.operational_checks,
            }).map(([key, pass]) => (
              <div key={key}>
                <span>{checkNames[key] || key}</span>
                <b className={pass ? "positive" : "warning"}>
                  {pass ? "通过" : "未通过"}
                </b>
              </div>
            ))}
          </div>
          <details className="advanced-settings">
            <summary>窗口与市场状态明细</summary>
            <div className="table-scroll">
              <table>
                <thead>
                  <tr>
                    <th>分组</th>
                    <th>样本 / 筛选</th>
                    <th>ECE</th>
                    <th>实际净优势</th>
                    <th>回撤</th>
                  </tr>
                </thead>
                <tbody>
                  {[
                    ...run.comparison.windows.map(
                      (w) => [`窗口 ${w.window}`, w.candidate] as const,
                    ),
                    ...Object.entries(run.comparison.regimes),
                  ].map(([name, m]) => (
                    <tr key={name}>
                      <td>{name}</td>
                      <td>
                        {m.samples} / {m.selected}
                      </td>
                      <td>{percent(m.ece)}</td>
                      <td>{percent(m.observed_net_edge)}</td>
                      <td>{percent(m.drawdown)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </details>
          <div className="settings-footer">
            <button
              className="primary-button"
              type="button"
              disabled={disabled || !run.promotion_ready}
              onClick={() => command("shadow_promote", { run_id: run.run_id })}
            >
              检查并晋升候选模型
            </button>
            <button
              type="button"
              className="text-button"
              onClick={exportEvidence}
            >
              导出对比证据
            </button>
          </div>
        </>
      )}
      {state.rollback_available && (
        <div className="settings-footer">
          <span className="muted">
            可恢复上一个模型；回退会结束当前预测并保持观察。
          </span>
          <button
            type="button"
            className="text-button"
            disabled={
              disabled ||
              state.mode !== "OBSERVE" ||
              Object.keys(state.positions).length > 0 ||
              !state.storage_healthy
            }
            onClick={() =>
              command("model_rollback", {
                current_hash: state.model.artifact_hash,
              })
            }
          >
            回退上一个模型
          </button>
        </div>
      )}
    </section>
  );
}
