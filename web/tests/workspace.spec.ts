import { test, expect } from "@playwright/test";
import completedOutcome from "./fixtures/completed-outcome.json";

test("trigger diagnostics preserve card semantics and submit versioned config only", async ({
  page,
  request,
}) => {
  const state = await (await request.get("/api/state")).json();
  state.trigger_config = {
    sample_interval_ms: 250,
    abnormal_z: 3,
    extreme_z: 6,
  };
  state.trigger_config_version = 1;
  state.symbols.BTCUSDT.runtime_state = "IDLE";
  state.symbols.BTCUSDT.full_evaluations = 0;
  state.mode = "OBSERVE";
  let submitted: unknown;
  await page.route("**/api/state", (route) => route.fulfill({ json: state }));
  await page.route("**/api/control/trigger_config", (route) => {
    submitted = route.request().postDataJSON();
    state.trigger_config = submitted;
    state.trigger_config_version += 1;
    return route.fulfill({ json: state });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "模型健康", exact: true }).click();
  const panel = page.getByTestId("trigger-diagnostics");
  await panel.getByText("运行触发诊断与配置", { exact: true }).click();
  await expect(panel).toContainText("IDLE");
  await expect(panel).toContainText("不代表买卖信号");
  await panel.getByRole("button", { name: "载入当前触发配置" }).click();
  await panel
    .getByLabel("触发配置 JSON")
    .fill(JSON.stringify({ ...state.trigger_config, extreme_z: 7 }));
  await panel
    .getByRole("button", { name: "保存触发配置（仅 OBSERVE 空仓）" })
    .click();
  await expect
    .poll(() => submitted)
    .toEqual({ sample_interval_ms: 250, abnormal_z: 3, extreme_z: 7 });
  await expect(panel.getByText(/fixed.step|固定.*扫描/)).toHaveCount(0);
});

test("live dashboard renders core data and switches instruments", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/");
  await expect(page.getByText("核心已连接", { exact: true })).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "市场工作台", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "核心判断", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "CONFIRM 确认交易 · 未就绪" }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "ETHUSDT 永续合约" }).click();
  await expect(page.locator(".market-price")).toContainText("ETHUSDT");
  expect(errors).toEqual([]);
  await page.screenshot({
    path: "../docs/screenshots/workspace-desktop.png",
    fullPage: true,
  });
});

test("paper mode, kill switch and risk settings are persisted through the API", async ({
  page,
  request,
}) => {
  const initial = await (await request.get("/api/state")).json();
  try {
    await page.goto("/");
    await expect(page.getByText("核心已连接", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "PAPER 模拟", exact: true }).click();
    await expect(
      page.getByRole("button", { name: "PAPER 模拟", exact: true }),
    ).toHaveClass(/selected/);
    await page.getByRole("button", { name: "紧急停止", exact: true }).click();
    await expect(page.getByText(/紧急停止已生效/)).toBeVisible();
    await page
      .getByRole("button", { name: "解除紧急停止", exact: true })
      .click();
    await expect(
      page.getByRole("button", { name: "紧急停止", exact: true }),
    ).toBeVisible();
    await page.getByRole("button", { name: "风险设置", exact: true }).click();
    const field = page.getByLabel("最大日亏损", { exact: true });
    const original = await field.inputValue();
    await field.fill("75");
    const saved = page.waitForResponse(
      (response) =>
        response.url().endsWith("/api/control/config") &&
        response.request().method() === "POST",
    );
    await page.getByRole("button", { name: "保存风险设置" }).click();
    expect((await saved).ok()).toBe(true);
    expect(
      (await (await request.get("/api/state")).json()).config.max_daily_loss,
    ).toBe(75);
    await expect(page.getByRole("status")).toContainText("已保存");
    await page.reload();
    await page.getByRole("button", { name: "风险设置", exact: true }).click();
    await expect(page.getByLabel("最大日亏损", { exact: true })).toHaveValue(
      "75",
    );
    await page.getByLabel("最大日亏损", { exact: true }).fill(original);
    await page.getByRole("button", { name: "保存风险设置" }).click();
    await expect(page.getByRole("status")).toContainText("已保存");
  } finally {
    const headers = { "X-Micro-Alpha": "local" };
    await request.post("/api/control/config", {
      headers,
      data: initial.config,
    });
    await request.post("/api/control/kill", {
      headers,
      data: { active: initial.kill_switch },
    });
    await request.post("/api/control/mode", {
      headers,
      data: { mode: initial.mode },
    });
  }
});

test("health, records and mobile layout stay usable", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("核心已连接", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "模型健康", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "概率与真实命中率" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "预测与交易", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "预测与交易记录" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "加载历史" }).click();
  await expect(page.getByRole("status")).toContainText("已加载");
  await page.getByRole("button", { name: "市场工作台", exact: true }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.getByRole("heading", { name: "核心判断" })).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({
    path: "../docs/screenshots/workspace-mobile.png",
    fullPage: true,
  });
});

test("direct API requests cannot bypass live gates or local request checks", async ({
  request,
}) => {
  const csrf = await request.post("/api/control/mode", {
    data: { mode: "PAPER" },
  });
  expect(csrf.status()).toBe(403);
  const live = await request.post("/api/control/mode", {
    headers: { "X-Micro-Alpha": "local" },
    data: { mode: "AUTO" },
  });
  expect(live.status()).toBe(400);
  const foreign = await request.post("/api/control/mode", {
    headers: { "X-Micro-Alpha": "local", Origin: "https://foreign.example" },
    data: { mode: "PAPER" },
  });
  expect(foreign.status()).toBe(403);
});

test("prediction card becomes its result and neutral state is explicit", async ({
  page,
  request,
}) => {
  const response = await request.get("/api/state");
  const state = await response.json();
  const s = state.symbols.BTCUSDT;
  const result = structuredClone(completedOutcome);
  s.active = null;
  s.last_result = result;
  s.lifecycle = "COMPLETED";
  s.signal_direction = "FLAT";
  s.qualification = null;
  s.plan = null;
  await page.route("**/api/state", (route) => route.fulfill({ json: state }));
  await page.goto("/");
  await expect(page.getByTestId("prediction-result")).toContainText("自动结果");
  await expect(page.locator(".recommendation")).toContainText(
    "已完成 · 上次判断",
  );
  await page.getByRole("button", { name: "查看结果详情", exact: true }).click();
  await expect(
    page.getByRole("dialog", { name: "预测结果详情" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  s.active = {
    ...result.prediction,
    prediction_id: "new-ui-fixture",
    ts: state.now,
    status: "TRACKING",
  };
  s.lifecycle = "TRACKING";
  await expect(page.getByTestId("prediction-result")).toHaveCount(0);
  await expect(page.locator(".recommendation")).toContainText("跟踪中");
  s.active = null;
  s.last_result = null;
  s.lifecycle = "NO_SIGNAL";
  await expect(page.locator(".recommendation")).toContainText(
    "FLAT · 中性观察",
  );
  await expect(page.getByTestId("prediction-lifecycle")).toContainText(
    "NO_SIGNAL",
  );
});

test("shadow import rejects invalid evidence without changing the current model", async ({
  page,
  request,
}) => {
  const before = await (await request.get("/api/state")).json();
  await page.goto("/");
  await expect(page.getByText("核心已连接", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "模型健康", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "影子模型与晋升" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "载入并开始影子验证" }),
  ).toBeDisabled();
  await page.getByLabel("候选模型文件").setInputFiles({
    name: "invalid-model.json",
    mimeType: "application/json",
    buffer: Buffer.from("{}"),
  });
  await page.getByLabel("离线报告文件").setInputFiles({
    name: "invalid-report.json",
    mimeType: "application/json",
    buffer: Buffer.from("{}"),
  });
  await page.getByRole("button", { name: "载入并开始影子验证" }).click();
  await expect(page.getByRole("status")).toContainText("模型 JSON 无效");
  const after = await (await request.get("/api/state")).json();
  expect(after.model.artifact_hash).toBe(before.model.artifact_hash);
  expect(after.shadow).toEqual(before.shadow);
  const bypass = await request.post("/api/control/shadow_promote", {
    headers: { "X-Micro-Alpha": "local" },
    data: { run_id: "unknown" },
  });
  expect(bypass.status()).toBe(400);
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({
    path: "../docs/screenshots/model-shadow.png",
    fullPage: true,
  });
});

test("shadow comparison disables promotion until ready and sends the reviewed run identity", async ({
  page,
  request,
}) => {
  const state = await (await request.get("/api/state")).json();
  const metrics = {
    samples: 0,
    selected: 0,
    coverage: 0,
    ece: 0,
    brier: 0,
    expected_net_edge: 0,
    observed_net_edge: 0,
    drawdown: 0,
    tail_p05: 0,
    strong_opposite_rate: 0,
  };
  state.shadow = {
    run_id: "ui-shadow-fixture",
    status: "RUNNING",
    started_at: state.now,
    config_version: state.config_version,
    champion_version: state.model.model_version,
    candidate_version: "ui-candidate",
    candidate_hash: "fixture-only",
    policy: {},
    offline_status: "PAPER_RESEARCH_GATE_PASS",
    operational_checks: { observe_mode: true },
    promotion_ready: false,
    comparison: {
      ready: false,
      checks: { samples: false },
      champion: metrics,
      candidate: metrics,
      champion_economics_comparable: false,
      symbols: { BTCUSDT: 0, ETHUSDT: 0 },
      invalid_rate: 0,
      pending: 2,
      retained: 0,
      windows: [],
      regimes: {},
    },
  };
  await page.route("**/api/state", (route) => route.fulfill({ json: state }));
  let submitted: unknown;
  await page.route("**/api/control/shadow_promote", async (route) => {
    submitted = route.request().postDataJSON();
    state.shadow.status = "PROMOTED";
    state.shadow.promotion_ready = false;
    state.model.model_version = "ui-candidate";
    state.mode = "OBSERVE";
    await route.fulfill({ json: state });
  });
  await page.goto("/");
  await expect(page.getByText("核心已连接", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "模型健康", exact: true }).click();
  const promote = page.getByRole("button", { name: "检查并晋升候选模型" });
  await expect(promote).toBeDisabled();
  await expect(page.getByTestId("shadow-gates")).toContainText("未通过");
  state.shadow.promotion_ready = true;
  await expect(promote).toBeEnabled();
  await promote.click();
  await expect.poll(() => submitted).toEqual({ run_id: "ui-shadow-fixture" });
  await expect(
    page.getByText("已晋升，保持观察", { exact: true }),
  ).toBeVisible();
  await expect(promote).toBeDisabled();
});

test("opening settings before the first response still initializes the form", async ({
  page,
  request,
}) => {
  const state = await (await request.get("/api/state")).json();
  let release!: () => void;
  const firstResponse = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/state", async (route) => {
    await firstResponse;
    await route.fulfill({ json: state });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "风险设置", exact: true }).click();
  release();
  await expect(page.getByLabel("最大日亏损", { exact: true })).toHaveValue(
    String(state.config.max_daily_loss),
  );
});

test("research controls create a challenger job without changing the champion", async ({
  page,
  request,
}) => {
  const state = await (await request.get("/api/state")).json();
  const version = state.model.model_version;
  let submitted: unknown;
  await page.route("**/api/state", (route) => route.fulfill({ json: state }));
  await page.route("**/api/control/research_run", (route) => {
    submitted = route.request().postDataJSON();
    state.research.status = "QUEUED";
    return route.fulfill({ json: state });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "模型健康", exact: true }).click();
  await page.getByLabel("候选模型", { exact: true }).selectOption("logistic");
  await page.getByRole("button", { name: "立即运行研究", exact: true }).click();
  await expect.poll(() => submitted).toEqual({ model: "logistic" });
  await expect(page.getByTestId("operations-panel")).toContainText("QUEUED");
  expect(state.model.model_version).toBe(version);
  await expect(
    page.getByRole("button", { name: "立即运行研究", exact: true }),
  ).toBeDisabled();
});

test("private execution reports cannot be forged from the browser; explanations are read only", async ({
  request,
}) => {
  const headers = { "X-Micro-Alpha": "local" };
  for (const command of [
    "broker_snapshot",
    "broker_order",
    "research_finished",
  ]) {
    const result = await request.post(`/api/control/${command}`, {
      headers,
      data: {},
    });
    expect(result.status()).toBe(403);
  }
  const before = await (await request.get("/api/state")).json();
  const result = await request.post("/api/assist", {
    headers,
    data: { symbol: "BTCUSDT" },
  });
  expect(result.ok()).toBeTruthy();
  expect((await result.json()).text).toContain("不修改概率");
  const after = await (await request.get("/api/state")).json();
  expect(after.model.model_version).toBe(before.model.model_version);
  expect(after.live.orders).toEqual(before.live.orders);
});

test("qualified episode displays independent AUTO skip and preserves opening probability", async ({
  page,
  request,
}) => {
  const state = await (await request.get("/api/state")).json();
  const s = state.symbols.BTCUSDT;
  const original =
    s.active ||
    state.outcomes.find(
      (o: { prediction: { symbol: string } }) =>
        o.prediction.symbol === "BTCUSDT",
    ).prediction;
  s.active = {
    ...original,
    prediction_id: "episode-ui-admission",
    ts: state.now,
    status: "TRACKING",
    calibrated_probabilities: [0.6, 0.3, 0.1],
    episode: {
      ...original.episode,
      latest_evaluated_at: state.now,
      latest_calibrated_probabilities: [0.7, 0.2, 0.1],
    },
  };
  s.qualification = { ...original.qualification, status: "QUALIFIED" };
  s.last_result = null;
  state.mode = "AUTO";
  const decision = {
    admission_id: "admission-ui-fixture",
    episode_id: s.active.prediction_id,
    symbol: "BTCUSDT",
    evaluated_at: state.now,
    evaluation_ts: state.now,
    decision: "SKIPPED",
    reason_codes: ["AUTO_EDGE_THRESHOLD_NOT_MET"],
    config_version: state.config_version,
    context: "LIVE",
    priority_tuple: {
      policy_version: "priority-v1",
      net_edge_after_cost: 0.0003,
      model_trust: 3,
      execution_quality: -0.001,
      risk_consumption: 1,
      episode_opened_at: state.now,
    },
    risk_budget_before: {
      available_loss: 100,
      available_notional: 100,
      available_slots: 1,
    },
    risk_budget_after_if_admitted: {
      available_loss: 100,
      available_notional: 100,
      available_slots: 1,
    },
    competing_episode_ids: [],
  };
  state.admission = { latest: { BTCUSDT: decision } };
  await page.route("**/api/state", (route) => route.fulfill({ json: state }));
  await page.goto("/");
  await expect(page.locator(".qualification")).toContainText("QUALIFIED");
  await expect(page.getByTestId("admission-card")).toContainText(
    "AUTO: SKIPPED",
  );
  await expect(page.getByTestId("admission-card")).toContainText(
    "AUTO_EDGE_THRESHOLD_NOT_MET",
  );
  await expect(page.getByTestId("episode-snapshots")).toContainText(
    "60.0% / 30.0% / 10.0%",
  );
  await expect(page.locator(".probability-labels")).toContainText("70.0%");
  state.admission.latest.BTCUSDT.decision = "ADMITTED";
  state.admission.latest.BTCUSDT.reason_codes = [];
  await expect(page.getByTestId("admission-card")).toContainText(
    "AUTO: ADMITTED",
  );
  await expect(page.getByTestId("admission-card")).toContainText(
    "成交以交易所回报为准",
  );
});

test("integrity counts and durable admission history are separate from prediction samples", async ({
  page,
  request,
}) => {
  const state = await (await request.get("/api/state")).json();
  state.daily_integrity = {
    realtime_evaluations: 600,
    prediction_episodes_opened: 5,
    prediction_episodes_closed: 4,
    qualified_episodes: 2,
    paper_trades: 1,
    live_trades: 0,
  };
  await page.route("**/api/state", (route) => route.fulfill({ json: state }));
  await page.route("**/api/history?kind=auto_admission*", (route) =>
    route.fulfill({
      json: [
        {
          payload: {
            admission_id: "audit-ui",
            symbol: "BTCUSDT",
            context: "SHADOW",
            episode_id: "episode-1",
            decision: "SKIPPED",
            reason_codes: ["REENTRY_COOLDOWN"],
          },
        },
      ],
    }),
  );
  await page.goto("/");
  await page.getByRole("button", { name: "模型健康", exact: true }).click();
  const panel = page.getByTestId("integrity-panel");
  await expect(panel).toContainText("实时评估 600");
  await expect(panel).toContainText("开启 5");
  await expect(panel).toContainText("交易所完成交易 0");
  await page.getByRole("button", { name: "加载准入审计" }).click();
  await expect(panel).toContainText("REENTRY_COOLDOWN");
});

test("AUTO settings enforce independent threshold on the server", async ({
  page,
  request,
}) => {
  const initial = await (await request.get("/api/state")).json();
  const bad = {
    ...initial.config,
    auto_min_net_edge: initial.config.min_net_edge / 2,
  };
  const result = await request.post("/api/control/config", {
    headers: { "X-Micro-Alpha": "local" },
    data: bad,
  });
  expect(result.status()).toBe(400);
  await page.goto("/");
  await page.getByRole("button", { name: "风险设置", exact: true }).click();
  await expect(
    page.getByLabel("AUTO 最低成本后优势", { exact: true }),
  ).toHaveValue(String(initial.config.auto_min_net_edge * 100));
  await expect(
    page.getByLabel("AUTO 最低模型可信度", { exact: true }),
  ).toHaveValue(initial.config.auto_required_model_trust);
});

test("live risk shows stale account separately from confirmed protection and execution feedback", async ({
  page,
  request,
}) => {
  const state = await (await request.get("/api/state")).json();
  state.live.configured = true;
  state.live.venue = "binance";
  state.live.environment = "testnet";
  state.live_enabled = false;
  state.live_gate_reason = "账户快照过期，保护性退出处理中";
  state.ai_mode = "OFF";
  state.symbols.BTCUSDT.health.paper_samples = 40;
  state.symbols.BTCUSDT.health.paper_ev_drift = -0.001;

  state.live_positions = {
    BTCUSDT: {
      quantity: 0.001,
      account_quantity: 0,
      account_age_ms: 15000,
      reconciled: false,
      current_notional: 100,
      target_notional: 0,
      effective_leverage: 0.1,
      target_leverage: 0,
      entry_price: 100000,
      mark_price: 99000,
      unrealized_pnl: -1,
      liquidation_buffer: 0.4,
      sl: 99000,
      tp: 102000,
      deadline_ts: state.now + 60000,
      action: "EXIT",
      reason: "止盈或止损触发",
      pending: [{ action: "EXIT", status: "UNKNOWN" }],
      feedback: {
        samples: 3,
        excluded_samples: 2,
        execution_drift: 0.001,
        ev_drift: -0.002,
        status: "DEGRADED",
      },
    },
  };
  await page.route("**/api/state", (route) => route.fulfill({ json: state }));
  await page.goto("/");
  await page.getByRole("button", { name: "模型健康", exact: true }).click();
  await expect(
    page.getByText("PAPER 实际净收益偏差", { exact: true }),
  ).toBeVisible();
  await expect(page.getByTestId("operations-panel")).toContainText(
    "AI 辅助：OFF",
  );
  const risk = page.getByTestId("live-position-risk");
  await expect(risk).toContainText("100.00 / 0.00 USDT");
  await expect(risk).toContainText("数量待对账，禁止新增风险");
  await expect(risk).toContainText("EXIT UNKNOWN");
  await expect(risk).toContainText("可比样本 3 · 排除 2");
  await expect(risk).toContainText("成本偏差 10.00 bps");
  await expect(
    page.getByRole("button", { name: "执行当前计划" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "减仓平掉 BTCUSDT" }),
  ).toBeEnabled();
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth + 1,
    ),
  ).toBeTruthy();
});
