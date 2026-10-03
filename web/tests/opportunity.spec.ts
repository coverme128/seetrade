import { test, expect } from "@playwright/test";
import type { Outcome, Prediction, State } from "../app/types";
import {
  currentOpportunity,
  openingOpportunity,
  sortOutcomes,
} from "../app/opportunity";

function prediction(id: string, trust = "HIGH", edge = 0.001): Prediction {
  return {
    prediction_id: id,
    symbol: "BTCUSDT",
    ts: Date.now() - 1000,
    horizon: 60,
    direction: "LONG",
    target: 0.003,
    stop: 0.001,
    start_price: 10000,
    current_price: 10000,
    expected_move: 0.001,
    signal_strength: 0.7,
    raw_probabilities: [0.9, 0.05, 0.05],
    calibrated_probabilities: [0.7, 0.2, 0.1],
    model_version: "quality-test",
    status: "TRACKING",
    mfe: 0,
    mae: 0,
    config_version: 1,
    plan: null,
    qualification: {
      prediction_id: id,
      status: "QUALIFIED",
      net_edge: edge,
      gross_edge: edge + 0.001,
      costs: {
        spread: 0,
        entry_fee: 0.0004,
        exit_fee: 0.0004,
        slippage: 0.0002,
        impact: 0,
        adverse_selection: 0,
        funding: 0,
        total: 0.001,
      },
      reasons: [],
      gates: [
        {
          code: "calibration",
          name: "概率校准",
          pass: true,
          detail: "校准通过",
        },
        { code: "model_trust", name: "模型可信度", pass: true, detail: trust },
        { code: "net_ev", name: "成本后 Net EV", pass: true, detail: "通过" },
      ],
    },
  };
}
function outcome(p: Prediction, result = "STOP_HIT"): Outcome {
  const hit = result === "TARGET_HIT";
  const move = hit ? p.target : -p.stop;
  return {
    prediction: p,
    outcome_type: result,
    first_hit_ts: p.ts + 1000,
    final_move: move,
    mfe: hit ? p.target : 0.0001,
    mae: hit ? 0.0001 : p.stop,
    prediction_error: "fixture",
    brier: 0.2,
    observed_net_edge: move - p.qualification.costs.total,
    learning_eligible: true,
    annotation: null,
  };
}
function install(state: State, symbol: string, p: Prediction, trust = "HIGH") {
  const s = state.symbols[symbol];
  p.symbol = symbol;
  s.active = p;
  s.lifecycle = "TRACKING";
  s.signal_direction = p.direction;
  s.qualification = p.qualification;
  s.health.trust = trust;
  s.evaluation = {
    evaluated_at: state.now,
    qualification_evaluated_at: state.now,
  };
  for (const v of Object.values(s.venues)) {
    v.ready = true;
    v.connected = true;
    v.age_ms = 0;
    v.exchange_age_ms = 0;
  }
}
function refreshFixture(state: State) {
  state.now = Date.now();
  for (const s of Object.values(state.symbols)) {
    if (s.evaluation)
      s.evaluation = {
        evaluated_at: state.now,
        qualification_evaluated_at: state.now,
      };
  }
}

test("quality uses qualification and trust, never raw probability or future outcomes", () => {
  const p = prediction("high");
  expect(openingOpportunity(outcome(p)).level).toBe("priority");
  expect(openingOpportunity(outcome(p, "TARGET_HIT"))).toEqual(
    openingOpportunity(outcome(p)),
  );
  expect(
    openingOpportunity(outcome(prediction("degraded", "DEGRADED"))).level,
  ).toBe("qualified");
  p.calibrated_probabilities = null;
  p.raw_probabilities = [0.999, 0.0005, 0.0005];
  expect(openingOpportunity(outcome(p, "TARGET_HIT")).level).toBe("unverified");
  p.calibrated_probabilities = [0.7, 0.2, 0.1];
  p.qualification.gates[1].detail = "LOW";
  expect(openingOpportunity(outcome(p)).level).toBe("unverified");
  p.qualification.gates[1].detail = "HIGH";
  p.qualification.gates[2].pass = false;
  expect(openingOpportunity(outcome(p)).level).toBe("blocked");
  p.qualification.gates = [];
  expect(openingOpportunity(outcome(p)).level).toBe("inactive");
});

test("sorting keeps qualified history first and never mutates source order", () => {
  const low = prediction("raw");
  low.calibrated_probabilities = null;
  low.ts = 300;
  const high = prediction("high", "HIGH", 0.0006);
  high.ts = 100;
  const better = prediction("better", "HIGH", 0.001);
  better.ts = 50;
  const degraded = prediction("degraded", "DEGRADED", 0.002);
  degraded.ts = 200;
  const rows = [low, high, better, degraded].map((p) => outcome(p));
  const ids = (r: Outcome[]) => r.map((o) => o.prediction.prediction_id);
  expect(ids(sortOutcomes(rows, "quality"))).toEqual([
    "better",
    "high",
    "degraded",
    "raw",
  ]);
  expect(ids(sortOutcomes(rows, "time"))).toEqual([
    "raw",
    "degraded",
    "high",
    "better",
  ]);
  expect(ids(rows)).toEqual(["raw", "high", "better", "degraded"]);
});

test("current quality expires and suppresses kill, model, data and config failures", async ({
  request,
}) => {
  const state: State = await (await request.get("/api/state")).json();
  state.now = Date.now();
  state.config_version = 1;
  state.config.max_stale_ms = 3000;
  state.kill_switch = false;
  state.storage_healthy = true;
  state.model.validated = true;
  state.model.model_version = "quality-test";
  install(state, "BTCUSDT", prediction("current"));
  const quality = (s = state, now = state.now, connected = true) =>
    currentOpportunity(s, "BTCUSDT", connected, now).level;
  expect(quality()).toBe("priority");
  expect(quality(state, state.now + 3001)).toBe("inactive");
  expect(quality(state, state.now, false)).toBe("inactive");
  for (const modify of [
    (s: State) => {
      s.kill_switch = true;
    },
    (s: State) => {
      s.storage_healthy = false;
    },
    (s: State) => {
      s.config_version = 2;
    },
    (s: State) => {
      s.symbols.BTCUSDT.venues.okx.ready = false;
    },
    (s: State) => {
      s.symbols.BTCUSDT.evaluation!.qualification_evaluated_at -= 4000;
    },
    (s: State) => {
      s.symbols.BTCUSDT.active!.ts -= 60000;
    },
  ]) {
    const s = structuredClone(state);
    modify(s);
    expect(quality(s)).toBe("inactive");
  }
  state.model.validated = false;
  expect(quality()).toBe("unverified");
});

test("both screens highlight live opportunities and sort opening history independently of results", async ({
  page,
  request,
}) => {
  const state: State = await (await request.get("/api/state")).json();
  state.now = Date.now();
  state.config_version = 1;
  state.source = "demo";
  state.kill_switch = false;
  state.storage_healthy = true;
  state.model.validated = true;
  state.model.model_version = "quality-test";
  install(state, "BTCUSDT", prediction("btc-low", "LOW"), "LOW");
  install(state, "ETHUSDT", prediction("eth-high"));
  state.symbols.ETHUSDT.active!.direction = "SHORT";
  state.symbols.ETHUSDT.active!.calibrated_probabilities = [0.2, 0.7, 0.1];
  state.symbols.ETHUSDT.signal_direction = "SHORT";
  const old = prediction("historical-priority");
  old.ts -= 60000;
  const latest = prediction("historical-raw");
  latest.ts -= 10000;
  latest.calibrated_probabilities = null;
  state.outcomes = [outcome(latest, "TARGET_HIT"), outcome(old, "STOP_HIT")];
  let writes = 0;
  await page.route("**/api/control/**", (route) => {
    writes++;
    return route.fulfill({ status: 400, json: {} });
  });
  await page.route("**/api/state", (route) => {
    refreshFixture(state);
    return route.fulfill({ json: state });
  });
  await page.goto("/");
  const panel = page.getByTestId("live-opportunities");
  await expect(panel.locator("button").first()).toHaveAttribute(
    "data-testid",
    "opportunity-ETHUSDT",
  );
  await expect(panel.getByTestId("opportunity-notice")).toHaveText(
    "优先关注：ETH",
  );
  await expect(page.getByTestId("opportunity-BTCUSDT")).toHaveAttribute(
    "data-level",
    "unverified",
  );
  await expect(page.getByTestId("opportunity-ETHUSDT")).toContainText("SHORT");
  const table = page.getByTestId("opportunity-history-table");
  await expect(table.locator("tbody tr").first()).toHaveAttribute(
    "data-episode-id",
    "historical-priority",
  );
  await expect(table.locator("tbody tr").first()).toContainText("止损先触达");
  await page.getByLabel("预测记录排序").selectOption("time");
  await expect(table.locator("tbody tr").first()).toHaveAttribute(
    "data-episode-id",
    "historical-raw",
  );
  await page.getByRole("button", { name: "预测与交易", exact: true }).click();
  await expect(page.getByLabel("预测记录排序")).toHaveValue("time");
  await expect(panel.getByTestId("opportunity-notice")).toHaveText(
    "优先关注：ETH",
  );
  await page.getByLabel("预测记录排序").selectOption("quality");
  await expect(table.locator("tbody tr").first()).toHaveAttribute(
    "data-level",
    "priority",
  );
  await page.screenshot({
    path: "../docs/screenshots/opportunity-history.png",
    fullPage: true,
  });
  await page.getByTestId("opportunity-ETHUSDT").click();
  await expect(
    page.getByRole("button", { name: "ETHUSDT 永续合约" }),
  ).toHaveClass("selected");
  await page.getByRole("button", { name: "市场工作台", exact: true }).click();
  await expect(page.getByTestId("prediction-opportunity-grade")).toContainText(
    "优先机会",
  );
  await page.screenshot({
    path: "../docs/screenshots/opportunity-overview.png",
    fullPage: true,
  });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({
    path: "../docs/screenshots/opportunity-mobile.png",
    fullPage: true,
  });
  state.symbols.ETHUSDT.health.trust = "LOW";
  await expect(panel.getByTestId("opportunity-notice")).toHaveText(
    "当前暂无优先机会",
  );
  await expect(page.getByTestId("opportunity-ETHUSDT")).toHaveAttribute(
    "data-level",
    "unverified",
  );
  expect(writes).toBe(0);
});

test("a frozen response loses its priority highlight even while HTTP remains successful", async ({
  page,
  request,
}) => {
  const state: State = await (await request.get("/api/state")).json();
  state.now = Date.now();
  state.config_version = 1;
  state.config.max_stale_ms = 3000;
  state.kill_switch = false;
  state.storage_healthy = true;
  state.model.validated = true;
  state.model.model_version = "quality-test";
  install(state, "BTCUSDT", prediction("frozen"));
  state.symbols.ETHUSDT.active = null;
  await page.route("**/api/state", (route) => route.fulfill({ json: state }));
  await page.goto("/");
  await expect(page.getByTestId("opportunity-BTCUSDT")).toHaveAttribute(
    "data-level",
    "priority",
  );
  await expect(page.getByTestId("opportunity-BTCUSDT")).toHaveAttribute(
    "data-level",
    "inactive",
    { timeout: 6000 },
  );
  await expect(
    page.getByTestId("live-opportunities").getByTestId("opportunity-notice"),
  ).toHaveText("当前暂无优先机会");
});
