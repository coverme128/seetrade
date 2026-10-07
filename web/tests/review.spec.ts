import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
const summary = {
  opportunities: 610,
  qualified: 0,
  pending: 1,
  eligible: 10,
  invalid: 599,
  paper_entries: 0,
  exchange_filled_opportunities: 0,
  observed_net_edge: -0.001,
  target_hit_rate: 0,
  calibration_samples: 0,
  calibration_ece: null,
  hit_probability_brier: null,
  outcomes: { SIGNAL_INVALIDATED: 599, TIMEOUT_POSITIVE: 10, PENDING: 1 },
  invalid_reasons: { confirmed_signal_change: 599 },
  execution: { OBSERVE_ONLY: 610 },
  non_execution_reasons: {},
};
function fixture(offset = 0, all = false) {
  const rows = Array.from(
    { length: all ? 610 : Math.min(50, 610 - offset) },
    (_, i) => ({
      prediction_id: `review-p-${offset + i}`,
      source: "live",
      symbol: "BTCUSDT",
      opened_at: Date.now() - 60000,
      direction: "LONG",
      model_version: "unvalidated",
      config_version: 35,
      opening_grade: "unverified",
      ever_qualified: false,
      outcome: "SIGNAL_INVALIDATED",
      learning_eligible: false,
      invalid_reason: "confirmed_signal_change",
      observed_net_edge: null,
      execution: "OBSERVE_ONLY",
      reasons: [],
      horizon_observation: { status: "OBSERVED", final_move_at_timeout: 0.01 },
      closed_trades: [],
      orders: [],
      shadow_admission: { decision: "SKIPPED", reasons: ["LOW_TRUST"] },
    }),
  );
  return {
    schema: "daily-review-v1",
    date: "2026-10-07",
    timezone: "Asia/Shanghai",
    as_of: 1791331200000,
    through_id: 999,
    latest_record_ts: 1791331200000,
    provisional: true,
    summary,
    cohorts: [
      {
        source: "live",
        symbol: "BTCUSDT",
        model_version: "unvalidated",
        config_version: 35,
        metrics: summary,
      },
    ],
    trading: [],
    trade_cohorts: [],
    execution: [],
    closed_trades: [],
    diagnostics: [
      {
        code: "HIGH_ATTRITION",
        message: "失效结果多于有效结果，先核对方向变化与数据中断。",
      },
      {
        code: "NO_CLOSED_TRADES",
        message: "尚无已平仓成交验证，不能将零成交判为策略通过。",
      },
    ],
    rows,
    pagination: { offset, limit: 50, total: 610 },
    limitations: ["观察净优势不是成交损益；独立到期观察不修改原标签。"],
  };
}
test("review shows exclusions, pending outcomes and no invented trading profit", async ({
  page,
}) => {
  await page.route("**/api/review?**", (route) =>
    route.fulfill({ json: fixture() }),
  );
  await page.goto("/");
  await page.getByRole("button", { name: "每日复盘", exact: true }).click();
  const panel = page.getByTestId("daily-review");
  await expect(panel).toContainText("共 610 条");
  await expect(panel).toContainText("尚无已平仓成交验证");
  await expect(panel).toContainText("确认方向变化");
  await expect(panel).toContainText("待完成");
  await panel.getByText("展开证据", { exact: true }).first().click();
  await expect(panel).toContainText("仅影子准入：SKIPPED（不代表执行）");
  await expect(panel).toContainText("独立到期观察：1.00%");
  await panel.getByText("按币种、模型与配置版本比较", { exact: false }).click();
  await expect(panel).toContainText("校准误差 / 样本");
});
test("pagination and full export pin the same evidence snapshot", async ({
  page,
}) => {
  const queries: URLSearchParams[] = [];
  await page.route("**/api/review?**", (route) => {
    const q = new URL(route.request().url()).searchParams;
    queries.push(q);
    return route.fulfill({
      json: fixture(Number(q.get("offset")), q.get("export") === "true"),
    });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "每日复盘", exact: true }).click();
  const panel = page.getByTestId("daily-review");
  await expect(panel).toContainText("共 610 条");
  await panel.getByRole("button", { name: "下一页", exact: true }).click();
  await expect(panel).toContainText("第 2 / 13 页");
  expect(queries.at(-1)?.get("through_id")).toBe("999");
  expect(queries.at(-1)?.get("as_of")).toBe("1791331200000");
  const downloadPromise = page.waitForEvent("download");
  await panel.getByRole("button", { name: "导出完整 JSON" }).click();
  const download = await downloadPromise;
  const exported = JSON.parse(await readFile((await download.path())!, "utf8"));
  expect(exported.rows).toHaveLength(610);
  expect(queries.at(-1)?.get("export")).toBe("true");
  await panel.getByLabel("复盘范围").selectOption("all");
  await expect.poll(() => queries.at(-1)?.get("date")).toBe("all");
  expect(queries.at(-1)?.has("through_id")).toBe(false);
});
test("mobile review stays readable and failures are explicit", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.route("**/api/review?**", (route) =>
    route.fulfill({ json: fixture() }),
  );
  await page.goto("/");
  await page.getByRole("button", { name: "每日复盘", exact: true }).click();
  const panel = page.getByTestId("daily-review");
  await expect(panel).toContainText("共 610 条");
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth + 2,
    ),
  ).toBeTruthy();
  await page.route("**/api/review?**", (route) =>
    route.fulfill({ status: 500, json: { error: "数据库暂不可用" } }),
  );
  await panel.getByRole("button", { name: "刷新复盘" }).click();
  await expect(panel.getByRole("alert")).toContainText("数据库暂不可用");
  await expect(
    panel.getByRole("button", { name: "导出完整 JSON" }),
  ).toBeDisabled();
});
test("real review endpoint validates queries and leaves trading configuration unchanged", async ({
  request,
}) => {
  const before = await (await request.get("/api/state")).json();
  const response = await request.get(
    `/api/review?date=all&source=${before.source}&limit=5`,
  );
  expect(response.status()).toBe(200);
  const report = await response.json();
  expect(report.schema).toBe("daily-review-v1");
  expect(report.timezone).toBe("Asia/Shanghai");
  expect(report.rows.length).toBeLessThanOrEqual(5);
  expect((await request.get("/api/review?date=2026-02-30")).status()).toBe(400);
  const after = await (await request.get("/api/state")).json();
  expect(after.mode).toBe(before.mode);
  expect(after.config_version).toBe(before.config_version);
  expect(after.model).toEqual(before.model);
});
