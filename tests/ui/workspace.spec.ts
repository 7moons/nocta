import { expect, test } from "@playwright/test";

test("empty workspace and keyboard-accessible import at 1280×720", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Account Manager" }),
  ).toBeVisible();
  await expect(page.getByText("No accounts here yet")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Launch selected" }),
  ).toBeDisabled();
  await page
    .getByRole("button", { name: "Add accounts", exact: true })
    .first()
    .click();
  await expect(
    page.getByRole("dialog", { name: "Add accounts" }),
  ).toBeVisible();
  await expect(
    page.getByRole("textbox", { name: "Accounts", exact: true }),
  ).toBeFocused();
  await page
    .getByRole("textbox", { name: "Accounts", exact: true })
    .fill("Preview:p:c");
  await page
    .getByRole("button", { name: "Import 1 account", exact: true })
    .click();
  await expect(page.getByRole("dialog").getByRole("alert")).toContainText(
    "read-only",
  );
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.getByRole("button", { name: "New folder" }).click();
  await expect(page.getByRole("textbox", { name: "New folder" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("textbox", { name: "New folder" })).toHaveCount(
    0,
  );
  await page.screenshot({
    path: "test-results/screenshots/accounts-empty-1280.png",
  });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    1280,
  );
  expect(errors).toEqual([]);
});

test("10,000 rows stay virtualized, scroll to last account, filter and select", async ({
  page,
}) => {
  await page.goto("/?benchmark=10000");
  const table = page.getByRole("table", { name: "Roblox accounts" });
  await expect(table).toHaveAttribute("aria-rowcount", "10001");
  expect(await table.getByRole("row").count()).toBeLessThan(80);
  await page.locator(".table-scroll").evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  await expect(
    page.getByText("PreviewAccount_10000", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("textbox", { name: "Search accounts" })
    .fill("PreviewAccount_10000, PreviewAccount_09999");
  await expect(table).toHaveAttribute("aria-rowcount", "3");
  await page
    .getByRole("checkbox", { name: "Select all visible accounts" })
    .check();
  await expect(
    page.getByText("2 selected", { exact: true }).first(),
  ).toBeVisible();
  await page.getByRole("button", { name: "Clear selection" }).click();
  await page.getByRole("button", { name: "Clear account search" }).click();
  await page
    .getByRole("combobox", { name: "Filter by session status" })
    .selectOption("invalid_session");
  await expect(table).toHaveAttribute("aria-rowcount", "771");
  await page
    .getByRole("combobox", { name: "Filter by session status" })
    .selectOption("all");
  await page.getByRole("button", { name: "Account", exact: true }).click();
  await page.getByRole("button", { name: "Account", exact: true }).click();
  await page
    .getByRole("textbox", { name: "Search accounts" })
    .fill("PreviewAccount_09999,PreviewAccount_10000");
  await expect(table.getByRole("row").nth(1)).toContainText(
    "PreviewAccount_09999",
  );
  await expect(table.getByRole("row").nth(2)).toContainText(
    "PreviewAccount_10000",
  );
  await page.getByRole("button", { name: "Clear account search" }).click();
  await expect(
    page.getByText("PreviewAccount_10000", { exact: true }),
  ).toBeVisible();
  await page.screenshot({
    path: "test-results/screenshots/accounts-benchmark-1280.png",
  });
  expect(await table.getByRole("row").count()).toBeLessThan(80);
});

test("settings, encrypted backup dialog and honest read-only browser preview", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  await expect(
    page.getByRole("spinbutton", { name: "Interface scale", exact: true }),
  ).toBeVisible();
  await page.screenshot({ path: "test-results/screenshots/settings-1280.png" });
  await page.getByRole("button", { name: "Backup", exact: true }).click();
  await page.setViewportSize({ width: 823, height: 884 });
  await page.screenshot({
    path: "test-results/screenshots/backup-settings-823.png",
  });
  await page.getByRole("button", { name: "Export", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Export encrypted backup" });
  await dialog
    .getByLabel("Backup password", { exact: true })
    .fill("synthetic-test-password");
  await dialog.getByLabel("Confirm password").fill("synthetic-test-password");
  await dialog.getByRole("button", { name: "Choose file" }).click();
  await expect(dialog.getByRole("alert")).toContainText("read-only");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Expand activity panel" }).click();
  await page.getByRole("button", { name: "Collapse activity panel" }).click();
  await expect(
    page.getByRole("button", { name: "Expand activity panel" }),
  ).toBeVisible();
});

test("snapshot events without accounts and logs keep the ones shown", async ({
  page,
}) => {
  const { mockDesktop } = await import("./desktopFixture");
  await mockDesktop(page, (snapshot) => {
    snapshot.logs = [
      {
        id: 1,
        timestamp: new Date().toISOString(),
        level: "INFO",
        module: "SYSTEM",
        message: "First log line",
        accountId: null,
      },
    ];
  });
  const rows = page.locator(".table-row");
  await expect(rows).toHaveCount(3);
  await expect(page.locator(".statusbar")).toHaveCount(1);
  // A task update: only the small parts change, accounts and logs are left out.
  await page.evaluate(() => {
    const emit = (
      window as unknown as {
        fixtureEmit: (patch: Record<string, unknown>, omit: string[]) => void;
      }
    ).fixtureEmit;
    emit({ config: { showStatusBar: false } }, ["accounts", "logs"]);
  });
  await expect(page.locator(".statusbar")).toHaveCount(0);
  await expect(rows).toHaveCount(3);
  await page.getByRole("button", { name: /^Logs/ }).click();
  await expect(page.locator(".log-message")).toHaveText("First log line");
});

test("the 7moons.dev credit opens the website", async ({ page }) => {
  const { mockDesktop, calls } = await import("./desktopFixture");
  await mockDesktop(page);
  const link = page.getByRole("button", { name: "7moons.dev" });
  await expect(link).toHaveAttribute(
    "title",
    "Open 7moons.dev in your browser",
  );
  await link.click();
  await expect
    .poll(async () => (await calls(page, "open_homepage")).length)
    .toBe(1);
  await page.locator(".titlebar").screenshot({
    path: "test-results/screenshots/titlebar-credit.png",
  });
});
