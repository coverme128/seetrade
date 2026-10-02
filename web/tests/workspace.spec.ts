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
}) => {
  await page.goto("/");
  await expect(page.getByText("核心已连接", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "PAPER 模拟", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "PAPER 模拟", exact: true }),
  ).toHaveClass(/selected/);
  await page.getByRole("button", { name: "紧急停止", exact: true }).click();
  await expect(page.getByText(/紧急停止已生效/)).toBeVisible();
  await page.getByRole("button", { name: "解除紧急停止", exact: true }).click();
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
