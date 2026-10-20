import { expect, test } from "@playwright/test";
import { calls, icon, lastArgs, mockDesktop } from "./desktopFixture";

test("FPS cap, AFK profile, auto-rejoin and the Discord webhook are set in Settings", async ({
  page,
}) => {
  await mockDesktop(page);
  await page.getByRole("button", { name: /^Settings/ }).click();
  await page
    .getByRole("button", { name: /Game launch/ })
    .first()
    .click();
  const performance = page.locator("#settings-performance");
  await expect(performance).toContainText("Performance and AFK");

  await performance.getByLabel("Frame-rate cap", { exact: true }).fill("60");
  await performance.getByLabel("Frame-rate cap", { exact: true }).blur();
  await expect
    .poll(async () => (await lastArgs(page, "patch_config"))?.patch)
    .toEqual({ fpsCap: 60 });
  await performance.getByLabel("AFK frame-rate cap").fill("10");
  await performance.getByLabel("AFK frame-rate cap").blur();
  await expect
    .poll(async () => (await lastArgs(page, "patch_config"))?.patch)
    .toEqual({ afkFpsCap: 10 });
  await performance
    .getByRole("switch", { name: "Minimize AFK windows" })
    .click();
  await expect
    .poll(async () => (await lastArgs(page, "patch_config"))?.patch)
    .toEqual({ afkMinimize: false });

  const recovery = page.locator("#settings-recovery");
  await recovery.getByRole("switch", { name: "Auto-rejoin" }).click();
  await expect
    .poll(async () => (await lastArgs(page, "patch_config"))?.patch)
    .toEqual({ autoRejoin: true });
  const webhook = recovery.getByLabel("Discord webhook");
  await webhook.fill("https://example.com/hook");
  await webhook.blur();
  await recovery.getByRole("button", { name: "Send test" }).click();
  await expect(recovery.getByRole("status")).toContainText(
    "Paste a Discord webhook link",
  );
  await webhook.fill("https://discord.com/api/webhooks/1/abc");
  await webhook.blur();
  await expect
    .poll(async () => (await lastArgs(page, "patch_config"))?.patch)
    .toEqual({ discordWebhook: "https://discord.com/api/webhooks/1/abc" });
  await recovery.getByRole("button", { name: "Send test" }).click();
  await expect(recovery.getByRole("status")).toHaveText("Test message sent.");
  await performance.scrollIntoViewIfNeeded();
  await page.screenshot({
    path: "test-results/screenshots/settings-performance-1280.png",
    fullPage: true,
  });
});

test("alias, color, pin, note and authenticator key are edited and shown in the table", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await mockDesktop(
    page,
    (snapshot) => {
      snapshot.accounts.forEach((account, index) => {
        account.userId = String(1000 + index);
        account.hasCookie = true;
        account.checkedAt = new Date().toISOString();
      });
      // Not checked for a month: flagged as an old session.
      snapshot.accounts[2].checkedAt = "2026-08-01T12:00:00Z";
    },
    { avatars: { "1001": icon("4f8bd6") } },
  );
  const rows = page.locator(".table-row");
  await expect(rows.nth(1).locator(".avatar img")).toHaveAttribute(
    "src",
    /data:image/,
  );
  await expect(rows.nth(2)).toContainText("Old session");
  await expect(rows.nth(0)).not.toContainText("Old session");

  // Edit the third account: alias, color, pin, note and an authenticator key.
  await rows.nth(2).dblclick();
  const dialog = page.getByRole("dialog", { name: "Edit account" });
  await dialog.getByLabel("Alias").fill("Main farm");
  await dialog.getByRole("radio", { name: "#22c55e" }).click();
  await dialog.getByRole("checkbox", { name: /Keep at the top/ }).check();
  await dialog.getByLabel("Note").fill("Daily rewards at 8:00");
  const totp = dialog.locator(".credential-field").filter({
    hasText: "Two-step verification",
  });
  await totp.getByRole("button", { name: "Add" }).click();
  await totp.getByLabel("Authenticator key").fill("GEZD GNBV GY3T QOJQ");
  await page.screenshot({
    path: "test-results/screenshots/account-edit-details-1280.png",
  });
  await dialog.getByRole("button", { name: "Save changes" }).click();
  await expect(dialog).toBeHidden();
  expect((await lastArgs(page, "update_account_details"))?.details).toEqual({
    alias: "Main farm",
    note: "Daily rewards at 8:00",
    color: "#22c55e",
    pinned: true,
    isMain: false,
    fields: [],
    afkPoint: null,
  });
  expect(await lastArgs(page, "set_totp")).toEqual({
    id: 3,
    key: "GEZD GNBV GY3T QOJQ",
  });
  // Only the details changed, so the account itself was not rewritten.
  expect(await calls(page, "update_account")).toHaveLength(0);

  // Pinned accounts come first and show their alias, real name and note.
  await expect(rows.first()).toContainText("Main farm");
  await expect(rows.first()).toContainText("PreviewAccount_00003");
  await expect(rows.first()).toContainText("Daily rewards at 8:00");
  await expect(rows.first().locator(".account-pin")).toBeVisible();
  await page.getByLabel("Search accounts").fill("main");
  await expect(rows).toHaveCount(1);
  await page.getByLabel("Search accounts").fill("");

  // Online status from Roblox: a dot on the avatar.
  await page.getByRole("button", { name: "Refresh online status" }).click();
  await expect(page.getByText("Online status of 3 accounts")).toBeVisible();
  await expect(page.locator(".presence-dot.in-game")).toHaveCount(1);
  await expect(page.locator(".presence-dot.online")).toHaveCount(2);
  await page.screenshot({
    path: "test-results/screenshots/accounts-details-presence-1280.png",
  });

  // The actions dialog shows the current code with a countdown and copies it.
  await page
    .getByRole("button", { name: "Actions for PreviewAccount_00003" })
    .click();
  const actions = page.getByRole("dialog", { name: "PreviewAccount_00003" });
  await expect(actions.locator(".totp-code")).toHaveText("492 039");
  await expect(actions).toContainText("Valid for 21 s");
  await actions.getByRole("button", { name: "Copy code" }).click();
  await expect(actions.getByRole("button", { name: "Copied" })).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    "492039",
  );
  await actions.screenshot({
    path: "test-results/screenshots/account-totp-code.png",
  });
});

test("copied credentials are cleared from the clipboard, other text is left alone", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await mockDesktop(page);
  const result = await page.evaluate(async () => {
    const { copySecret } = await import("/src/lib/clipboard.ts");
    const wait = (ms: number) => new Promise((done) => setTimeout(done, ms));
    await copySecret("secret-cookie", 150);
    const before = await navigator.clipboard.readText();
    await wait(300);
    const after = await navigator.clipboard.readText();
    await copySecret("second-secret", 150);
    await navigator.clipboard.writeText("something the user copied");
    await wait(300);
    const kept = await navigator.clipboard.readText();
    return { before, after, kept };
  });
  expect(result).toEqual({
    before: "secret-cookie",
    after: "",
    kept: "something the user copied",
  });
});

test("selected accounts export to CSV and a browser sign-in adds an account", async ({
  page,
}) => {
  await mockDesktop(page);
  await page.getByRole("checkbox", { name: "Select all" }).check();
  await page
    .getByLabel("Export selected accounts")
    .selectOption({ label: "CSV file" });
  await expect
    .poll(() => lastArgs(page, "export_accounts"))
    .toEqual({ ids: [1, 2, 3], format: "csv" });
  await expect(
    page.getByText("3 accounts exported with passwords and cookies."),
  ).toBeVisible();
  await page.locator(".bulk-actions").screenshot({
    path: "test-results/screenshots/accounts-export-bar.png",
  });

  await page.getByRole("button", { name: "Add accounts" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("Sign in with a browser");
  await dialog.screenshot({
    path: "test-results/screenshots/add-account-sign-in.png",
  });
  await dialog.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(dialog).toBeHidden();
  expect(await lastArgs(page, "sign_in_account")).toEqual({ folderId: null });
  await expect(page.getByText("SignedInFriend")).toBeVisible();
});

test("right click shows no browser menu, except in text fields", async ({
  page,
}) => {
  await mockDesktop(page);
  const prevented = (selector: string) =>
    page
      .locator(selector)
      .first()
      .evaluate((element) => {
        const event = new MouseEvent("contextmenu", {
          bubbles: true,
          cancelable: true,
        });
        element.dispatchEvent(event);
        return event.defaultPrevented;
      });
  expect(await prevented(".view-header")).toBe(true);
  expect(await prevented(".sidebar")).toBe(true);
  expect(await prevented(".accounts-search input")).toBe(false);
  // The app's own right-click menu on a row still opens.
  await page.locator(".table-row").first().click({ button: "right" });
  await expect(
    page.getByRole("dialog", { name: "PreviewAccount_00001" }),
  ).toBeVisible();
});

test("never-checked accounts are not flagged as old sessions", async ({
  page,
}) => {
  await mockDesktop(page, (snapshot) => {
    for (const account of snapshot.accounts) {
      account.hasCookie = true;
      account.health = "unchecked";
      account.checkedAt = null;
    }
  });
  await expect(page.locator(".table-row")).toHaveCount(3);
  await expect(page.locator(".stale-flag")).toHaveCount(0);
});
