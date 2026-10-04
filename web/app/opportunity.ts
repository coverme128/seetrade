import type { Outcome, Prediction, Qualification, State } from "./types";

export type Opportunity = {
  level: "priority" | "qualified" | "unverified" | "blocked" | "inactive";
  label: string;
  rank: number;
  reason: string;
  netEdge: number | null;
};
export type OpportunitySort = "quality" | "time";

function grade(
  level: Opportunity["level"],
  label: string,
  rank: number,
  reason: string,
  netEdge: number | null = null,
): Opportunity {
  return { level, label, rank, reason, netEdge };
}

// Display-only classification. Qualification / admission remain engine decisions.
function assess(
  p: Prediction,
  q: Qualification | null,
  probabilities: number[] | null | undefined,
  trust: string | undefined,
): Opportunity {
  if (!q || !q.gates?.length || q.prediction_id !== p.prediction_id)
    return grade(
      "inactive",
      "信息不足",
      0,
      "缺少匹配的资格快照，不能判断机会等级",
    );
  const calibrated =
    probabilities?.length === 3 &&
    probabilities.every((n) => Number.isFinite(n) && n >= 0 && n <= 1) &&
    Math.abs(probabilities.reduce((a, b) => a + b, 0) - 1) < 0.001;
  if (
    !calibrated ||
    !["HIGH", "DEGRADED"].includes(trust || "") ||
    !q.gates.some((g) => g.code === "calibration" && g.pass) ||
    !q.gates.some((g) => g.code === "model_trust" && g.pass)
  )
    return grade(
      "unverified",
      "待验证",
      2,
      "校准或模型可信度未通过；高原始概率不提升等级",
    );
  if (
    q.status !== "QUALIFIED" ||
    q.gates.some((g) => !g.pass) ||
    q.reasons.length > 0 ||
    !Number.isFinite(q.net_edge) ||
    q.net_edge <= 0
  )
    return grade(
      "blocked",
      "未达标",
      1,
      q.gates
        .filter((g) => !g.pass)
        .map((g) => g.name)
        .join("、") || "交易资格或成本后优势未通过",
    );
  return trust === "HIGH"
    ? grade(
        "priority",
        "优先机会",
        4,
        "交易资格全部通过，模型可信度 HIGH；执行仍需准入与复核",
        q.net_edge,
      )
    : grade(
        "qualified",
        "合格机会",
        3,
        "交易资格通过，模型可信度 DEGRADED；风险预算受限",
        q.net_edge,
      );
}

export function currentOpportunity(
  state: State,
  symbol: string,
  connected: boolean,
  wallNow: number,
): Opportunity {
  const s = state.symbols[symbol];
  const p = s?.active;
  const config = s?.config || state.config;
  if (!p) return grade("inactive", "暂无机会", 0, "当前没有跟踪中的预测");
  const fresh = (ts: number | undefined) =>
    ts !== undefined &&
    Number.isFinite(ts) &&
    Math.abs(wallNow - ts) <= config.max_stale_ms;
  if (
    !connected ||
    !fresh(state.now) ||
    !fresh(s.evaluation?.evaluated_at) ||
    !fresh(s.evaluation?.qualification_evaluated_at) ||
    p.config_version !== (s.config_version ?? state.config_version) ||
    wallNow >= p.ts + p.horizon * 1000 ||
    !["binance", "okx"].every((venue) => {
      const v = s.venues[venue];
      return (
        v?.ready &&
        v.connected &&
        v.age_ms <= config.max_stale_ms &&
        v.exchange_age_ms <= config.max_stale_ms
      );
    })
  )
    return grade(
      "inactive",
      "暂不可用",
      0,
      "连接、行情或资格评估已过期，等待最新有效数据",
    );
  if (state.kill_switch || !state.storage_healthy)
    return grade(
      "inactive",
      "已暂停",
      0,
      "紧急停止或存储异常，当前不突出新增机会",
    );
  const model = s.model || state.model;
  if (!model.validated || p.model_version !== model.model_version)
    return grade(
      "unverified",
      "待验证",
      2,
      "当前模型尚未验证，或预测与当前模型不匹配",
    );
  return assess(
    p,
    s.qualification,
    p.episode
      ? p.episode.latest_calibrated_probabilities
      : p.calibrated_probabilities,
    s.health.trust,
  );
}

export function openingOpportunity(outcome: Outcome): Opportunity {
  const p = outcome.prediction;
  // Never use the eventual outcome, latest qualification, current config or current model.
  return assess(
    p,
    p.qualification,
    p.calibrated_probabilities,
    p.qualification?.gates?.find((g) => g.code === "model_trust")?.detail,
  );
}

export function compareOpportunity(a: Opportunity, b: Opportunity): number {
  return b.rank - a.rank || (b.netEdge ?? 0) - (a.netEdge ?? 0);
}

export function sortOutcomes(
  rows: Outcome[],
  sort: OpportunitySort,
): Outcome[] {
  return [...rows].sort(
    (a, b) =>
      (sort === "quality"
        ? compareOpportunity(openingOpportunity(a), openingOpportunity(b))
        : 0) ||
      b.prediction.ts - a.prediction.ts ||
      a.prediction.prediction_id.localeCompare(b.prediction.prediction_id),
  );
}
