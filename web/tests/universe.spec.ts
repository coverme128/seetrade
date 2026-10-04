import { test, expect } from "@playwright/test";
import type { State } from "../app/types";
import type { AssetProfile, Universe } from "../app/universe-types";
function fixture(base: State): State {
  const state = structuredClone(base);
  state.now = Date.now();
  state.source = "demo";
  state.mode = "OBSERVE";
  const profile = (symbol: string, priority: string): AssetProfile => ({
    id: `asset-${symbol}-1`,
    asset_key: symbol,
    profile_version: 1,
    lifecycle_state: "DISCOVERED",
    supported_exchanges: ["binance", "okx"],
    mode_permissions: ["OBSERVE"],
    collecting: false,
    clean_since: null,
    good_scans: 0,
    bad_scans: 0,
    model: null,
    calibration_artifact_id: null,
    cost_artifact_id: null,
    config: null,
    feature_normalization_version: "warmup",
    trigger: {
      id: `trigger-${symbol}`,
      profile_version: 1,
      calibrated: false,
      metric_distribution_version: "unvalidated",
      config: {},
    },
    observation: { metrics: { quote_volume: 100000000, depth: null } },
    qualification: {
      overall_state: "CONDITIONAL",
      research_priority: priority,
      blocking_reasons: [],
      warnings: ["continuity"],
      next_action: "采集",
      components: { continuity: { state: "UNKNOWN", evidence: null } },
    },
  });
  state.universe = {
    config: {
      enabled: false,
      dry_run: true,
      interval_secs: 300,
      candidate_l2_slots: 12,
      minimum_clean_hours: 24,
      minimum_outcomes: 2000,
      persistence: 3,
      min_quote_volume: 10000000,
      max_spread_bps: 10,
      min_depth: 20000,
      max_slippage_bps: 10,
      max_volatility: 0.01,
      max_tail_gap: 0.03,
    },
    config_version: 1,
    frozen: false,
    requested: false,
    running: null,
    next_run: 0,
    last_run: {
      id: "scan-test",
      status: "PARTIAL",
      started_at: state.now,
      completed_at: state.now,
      considered: 3,
      errors: { okx: "公开行情暂时不可用" },
    },
    profiles: {
      BTCUSDT: profile("BTCUSDT", "MEDIUM"),
      SOLUSDT: profile("SOLUSDT", "HIGH"),
      BADUSDT: profile("BADUSDT", "REJECT"),
    },
    transitions: [],
    portfolio: { live_enabled: false, enforce_paper: true, max_positions: 3 },
    portfolio_latest: {},
  } satisfies Universe;
  state.portfolio_exposure = {
    equity: 10000,
    gross_notional: 0,
    long_notional: 0,
    short_notional: 0,
    risk_used: 0,
    positions: 0,
    effective_leverage: 0,
    venue: "paper",
  };
  for (const s of Object.values(state.symbols)) {
    s.active = null;
    s.runtime_state = "IDLE";
  }
  return state;
}
test("scanner ranks research separately and manual addition only requests validation", async ({
  page,
  request,
}) => {
  const state = fixture(await (await request.get("/api/state")).json());
  const calls: { name: string; payload: unknown }[] = [];
  await page.route("**/api/state", (r) => r.fulfill({ json: state }));
  await page.route("**/api/control/*", (r) => {
    calls.push({
      name: r.request().url().split("/").at(-1)!,
      payload: r.request().postDataJSON(),
    });
    return r.fulfill({ json: state });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "币种扫描器", exact: true }).click();
  await expect(
    page.getByTestId("scanner-table").locator("tbody tr").first(),
  ).toContainText("SOLUSDT");
  await expect(
    page.getByText("公开行情暂时不可用", { exact: false }),
  ).toBeVisible();
  await page.getByLabel("手动添加合约").fill("XRPUSDT");
  await page.getByRole("button", { name: "验证并扫描" }).click();
  expect(calls).toEqual([
    { name: "asset_add", payload: { symbol: "XRPUSDT" } },
  ]);
  await expect(page.getByTestId("scanner-table")).not.toContainText("买入");
  await expect(page.getByTestId("scanner-table")).toContainText("未知");
});
test("radar hides idle and empty pulses and exposes no direct trade action", async ({
  page,
  request,
}) => {
  const state = fixture(await (await request.get("/api/state")).json());
  state.symbols.BTCUSDT.runtime_state = "CANDIDATE";
  await page.route("**/api/state", (r) => r.fulfill({ json: state }));
  await page.goto("/");
  await page.getByRole("button", { name: "机会雷达", exact: true }).click();
  await expect(page.getByTestId("radar-card")).toHaveCount(0);
  await expect(
    page.getByText("暂无活跃交易机会，继续监听市场变化。"),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "执行交易", exact: true }),
  ).toHaveCount(0);
});
test("mobile scanner remains usable and shows frozen state", async ({
  page,
  request,
}) => {
  const state = fixture(await (await request.get("/api/state")).json());
  state.universe!.frozen = true;
  await page.setViewportSize({ width: 390, height: 844 });
  await page.route("**/api/state", (r) => r.fulfill({ json: state }));
  await page.goto("/");
  await page.getByRole("button", { name: "币种扫描器", exact: true }).click();
  await expect(page.getByRole("button", { name: "立即扫描" })).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "解除动态池冻结" }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
});
