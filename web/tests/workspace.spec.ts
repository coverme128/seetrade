import { test, expect } from "@playwright/test";

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
    page.getByRole("button", { name: "CONFIRM 确认交易 · 未开放" }),
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
    await page.getByRole("button", { name: "保存风险设置" }).click();
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
  const result = state.outcomes.find(
    (o: { prediction: { symbol: string } }) =>
      o.prediction.symbol === "BTCUSDT",
  );
  expect(
    result,
    "running core should have recorded a completed BTC prediction",
  ).toBeTruthy();
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
