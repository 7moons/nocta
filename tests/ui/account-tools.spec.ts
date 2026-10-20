import { expect, test, type Page } from "@playwright/test";
import type { AppSnapshot } from "../../src/generated/contracts";
import { calls, lastArgs, mockDesktop } from "./desktopFixture";

const lastPatch = async (page: Page) =>
  (await lastArgs(page, "patch_config"))?.patch;

/** Accounts with a Roblox ID and a session, so Roblox-side tools are offered. */
const signedIn = (snapshot: AppSnapshot) => {
  snapshot.accounts.forEach((account, index) => {
    account.userId = String(700 + index);
    account.hasCookie = true;
    account.checkedAt = new Date().toISOString();
  });
};

test("streamer mode hides usernames, Roblox IDs and names in logs; aliases stay", async ({
  page,
}) => {
  await mockDesktop(page, (snapshot) => {
    signedIn(snapshot);
    snapshot.accounts[1].alias = "Farm";
    snapshot.logs = [
      {
        id: 1,
        timestamp: new Date().toISOString(),
        level: "INFO",
        module: "LAUNCH",
        message: "PreviewAccount_00001's Roblox client was disconnected.",
        accountId: 1,
      },
    ];
  });
  expect((await calls(page, "ui_ready")).length).toBeGreaterThan(0);
  const table = page.getByRole("table", { name: /accounts/i });
  await expect(table).toContainText("PreviewAccount_00001");

  await page.getByRole("button", { name: /^Settings/ }).click();
  await page
    .getByRole("button", { name: /^Appearance/ })
    .first()
    .click();
  await page.getByRole("switch", { name: "Streamer mode" }).click();
  await expect.poll(() => lastPatch(page)).toEqual({ streamerMode: true });

  await page.getByRole("button", { name: /^Accounts/ }).click();
  await page.getByRole("button", { name: /^Logs/ }).click();
  await expect(table).toContainText("Account #1");
  await expect(table).toContainText("Farm");
  await expect(table).toContainText("Account #3");
  await expect(page.locator("body")).not.toContainText("PreviewAccount_0000");
  await expect(page.locator("body")).not.toContainText("Roblox ID 700");
  await expect(page.locator(".log-message").first()).toHaveText(
    "Account #1's Roblox client was disconnected.",
  );
  await page.screenshot({
    path: "test-results/screenshots/streamer-mode-1280.png",
  });

  // Ctrl+K turns it off again.
  await page.keyboard.press("Control+k");
  await page.keyboard.type("streamer");
  await page.keyboard.press("Enter");
  await expect.poll(() => lastPatch(page)).toEqual({ streamerMode: false });
  await expect(table).toContainText("PreviewAccount_00001");
});

test("own fields and an anti-AFK click point are saved, searchable and shown", async ({
  page,
}) => {
  await mockDesktop(page, signedIn);
  const rows = page.locator(".table-row");
  await rows.nth(1).dblclick();
  const dialog = page.getByRole("dialog", { name: "Edit account" });
  await dialog.getByRole("button", { name: "Add field" }).click();
  await dialog.getByLabel("Field 1 name").fill("Server");
  await dialog.getByLabel("Field 1 value").fill("EU-West");
  await dialog.getByRole("button", { name: "Add field" }).click();
  await dialog.getByLabel("Field 2 name").fill("Group");
  await dialog.getByLabel("Field 2 value").fill("Night farm");
  await dialog
    .getByRole("checkbox", { name: /Own anti-AFK click point/ })
    .check();
  await dialog.getByLabel("Click point X").fill("30");
  await dialog.getByLabel("Click point Y").fill("85");
  await dialog.locator(".account-fields").scrollIntoViewIfNeeded();
  await dialog.screenshot({
    path: "test-results/screenshots/account-fields-click-point.png",
  });
  await dialog.getByRole("button", { name: "Save changes" }).click();
  await expect(dialog).toBeHidden();
  expect((await lastArgs(page, "update_account_details"))?.details).toEqual({
    alias: "",
    note: "",
    color: null,
    pinned: false,
    isMain: false,
    fields: [
      { name: "Server", value: "EU-West" },
      { name: "Group", value: "Night farm" },
    ],
    afkPoint: { xPercent: 30, yPercent: 85 },
  });

  // Fields are found by value or as "name: value".
  await page.getByLabel("Search accounts").fill("eu-west");
  await expect(rows).toHaveCount(1);
  await expect(rows.first()).toContainText("PreviewAccount_00002");
  await page.getByLabel("Search accounts").fill("group: night");
  await expect(rows).toHaveCount(1);
  await page.getByLabel("Search accounts").fill("");

  await page
    .getByRole("button", { name: "Actions for PreviewAccount_00002" })
    .click();
  const facts = page.locator(".account-own-fields");
  await expect(facts).toContainText("ServerEU-West");
  await expect(facts).toContainText("GroupNight farm");
});

test("accounts are dragged into the user's own order", async ({ page }) => {
  await mockDesktop(page);
  const rows = page.locator(".table-row");
  await expect(rows.first()).toContainText("PreviewAccount_00001");
  await rows.nth(2).dragTo(rows.nth(0));
  await expect
    .poll(async () => (await lastArgs(page, "reorder_accounts"))?.ids)
    .toEqual([3, 1, 2]);
  await expect(rows.nth(0)).toContainText("PreviewAccount_00003");
  await expect(rows.nth(1)).toContainText("PreviewAccount_00001");
  // Down: the first row lands after the one it is dropped on.
  await rows.nth(0).dragTo(rows.nth(1));
  await expect
    .poll(async () => (await lastArgs(page, "reorder_accounts"))?.ids)
    .toEqual([1, 3, 2]);
  await expect(rows.nth(0)).toContainText("PreviewAccount_00001");
  await page.screenshot({
    path: "test-results/screenshots/accounts-own-order-1280.png",
  });
  // While a column sorts the list, rows still go onto folders but keep their own order.
  await page
    .getByRole("columnheader", { name: "Account" })
    .getByRole("button")
    .click();
  const before = (await calls(page, "reorder_accounts")).length;
  await rows.nth(1).dragTo(rows.nth(0));
  expect((await calls(page, "reorder_accounts")).length).toBe(before);
});

test("Roblox account tools: other sessions, display name, Quick Login and privacy", async ({
  page,
}) => {
  await mockDesktop(page, signedIn);
  await page
    .getByRole("button", { name: "Actions for PreviewAccount_00001" })
    .click();
  const dialog = page.getByRole("dialog", { name: "PreviewAccount_00001" });
  const tools = dialog.locator(".roblox-tools");
  await tools.scrollIntoViewIfNeeded();

  await tools.getByRole("tab", { name: "Display name" }).click();
  await tools.getByLabel("New display name").fill("Night Farmer");
  await tools.getByRole("button", { name: "Change" }).click();
  await expect(tools.getByRole("status")).toHaveText(
    "Display name changed to Night Farmer.",
  );
  expect(await lastArgs(page, "set_display_name")).toEqual({
    id: 1,
    name: "Night Farmer",
  });

  await tools.getByRole("tab", { name: "Quick Login" }).click();
  await tools.getByLabel("Quick Login code").fill("zzzzzz");
  await tools.getByRole("button", { name: "Check code" }).click();
  await expect(tools).toContainText("Roblox did not recognise this code.");
  await tools.getByLabel("Quick Login code").fill("ab3d9z");
  await tools.getByRole("button", { name: "Check code" }).click();
  await expect(tools).toContainText(
    "Chrome on Windows in Warsaw, Poland wants to sign in as this account.",
  );
  await dialog.screenshot({
    path: "test-results/screenshots/roblox-quick-login.png",
  });
  // Nothing is approved until the user says so.
  expect(await calls(page, "quick_login_approve")).toHaveLength(0);
  await tools.getByRole("button", { name: "Approve sign-in" }).click();
  await expect(tools.getByRole("status")).toHaveText(
    "The other device is signed in.",
  );
  expect(await lastArgs(page, "quick_login_approve")).toEqual({
    id: 1,
    code: "AB3D9Z",
  });

  await tools.getByRole("tab", { name: "Privacy" }).click();
  const join = tools.getByLabel("Who can join me in experiences");
  await expect(join).toHaveValue("Friends");
  await join.selectOption("AllUsers");
  await expect(tools.getByRole("status")).toHaveText("Saved on Roblox.");
  expect(await lastArgs(page, "set_privacy")).toEqual({
    ids: [1],
    key: "whoCanJoinMeInExperiences",
    value: "AllUsers",
  });
  // Everyone in chat needs Roblox's age check: Nocta warns before leaving it, as it cannot
  // switch back, and does nothing when the user says no.
  const chat = tools.getByLabel("Who can chat with me in experiences");
  await expect(chat).toHaveValue("AllUsers");
  await expect(
    chat.locator("option", { hasText: "Roblox check needed" }),
  ).toHaveCount(1);
  let warning = "";
  page.once("dialog", (confirm) => {
    warning = confirm.message();
    void confirm.dismiss();
  });
  await chat.selectOption("NoOne");
  await expect.poll(() => warning).toContain("Nocta cannot switch back");
  expect((await calls(page, "set_privacy")).length).toBe(1);
  await dialog.screenshot({
    path: "test-results/screenshots/roblox-privacy.png",
  });

  await tools.getByRole("tab", { name: "Other sessions" }).click();
  page.once("dialog", (confirm) => void confirm.accept());
  await tools.getByRole("button", { name: "Sign out everywhere else" }).click();
  await expect(tools.getByRole("status")).toHaveText(
    "Signed out everywhere else. Nocta keeps the new session.",
  );
  expect(await lastArgs(page, "sign_out_other_sessions")).toEqual({ id: 1 });
});

test("selected accounts become friends and get one privacy setting", async ({
  page,
}) => {
  await mockDesktop(page, (snapshot) => {
    signedIn(snapshot);
    snapshot.accounts[1].isMain = true;
  });
  await page.getByRole("checkbox", { name: "Select all" }).check();
  const operations = page.getByLabel("Run account operation");

  await operations.selectOption({ label: "Make them friends…" });
  const befriend = page.getByRole("dialog", {
    name: "Make the selected accounts friends",
  });
  await expect(befriend).toContainText("3 friendships to make.");
  await befriend
    .getByRole("radio", { name: "Every account with one main account" })
    .check();
  await expect(
    befriend.getByLabel("Main account", { exact: true }),
  ).toHaveValue("2");
  await expect(befriend).toContainText("2 friendships to make.");
  await befriend.screenshot({
    path: "test-results/screenshots/befriend-dialog.png",
  });
  await befriend.getByRole("button", { name: "Make friends" }).click();
  await expect(
    page.getByText(
      "2 new friendships, 1 already friends, 0 waiting for Roblox's check (run it again later), 0 failed.",
    ),
  ).toBeVisible();
  expect(await lastArgs(page, "befriend_accounts")).toEqual({
    ids: [1, 2, 3],
    mode: { kind: "with_main", main: 2 },
  });

  await operations.selectOption({ label: "Set privacy…" });
  const privacy = page.getByRole("dialog", {
    name: "Privacy for the selected accounts",
  });
  await privacy.getByLabel("Setting").selectOption("whoCanSeeMyInventory");
  await privacy.getByLabel("Value").selectOption("AllUsers");
  await privacy.getByRole("button", { name: "Apply to 3 accounts" }).click();
  await expect(
    page.getByText("Privacy changed on 3 of 3 accounts."),
  ).toBeVisible();
  expect(await lastArgs(page, "set_privacy")).toEqual({
    ids: [1, 2, 3],
    key: "whoCanSeeMyInventory",
    value: "AllUsers",
  });
});

test("anti-AFK can click a point, and GPU acceleration restarts Nocta", async ({
  page,
}) => {
  await mockDesktop(page);
  await page.getByRole("button", { name: /^Settings/ }).click();
  await page
    .getByRole("button", { name: /Game launch/ })
    .first()
    .click();
  const afk = page.locator("#settings-anti-afk");
  await afk.getByLabel("Key", { exact: true }).selectOption("click");
  await expect.poll(() => lastPatch(page)).toEqual({ antiAfkAction: "click" });
  await afk.getByLabel("Click point Y").fill("80");
  await afk.getByLabel("Click point Y").blur();
  await expect
    .poll(() => lastPatch(page))
    .toEqual({ antiAfkClick: { xPercent: 50, yPercent: 80 } });
  await afk.screenshot({
    path: "test-results/screenshots/settings-anti-afk-click.png",
  });

  await page
    .getByRole("button", { name: /^Appearance/ })
    .first()
    .click();
  // Drawing without the GPU is the default; turning acceleration on restarts Nocta with it.
  const gpu = page.getByRole("switch", { name: "GPU acceleration" });
  await expect(gpu).not.toBeChecked();
  page.once("dialog", (confirm) => void confirm.dismiss());
  await gpu.click();
  expect(await calls(page, "set_safe_display")).toHaveLength(0);
  page.once("dialog", (confirm) => void confirm.accept());
  await gpu.click();
  await expect
    .poll(() => lastArgs(page, "set_safe_display"))
    .toEqual({ on: false });
  await page.locator("#settings-display").screenshot({
    path: "test-results/screenshots/settings-streamer-safe-display.png",
  });
});

test("picked servers can be limited to a region", async ({ page }) => {
  await mockDesktop(page, (snapshot) => {
    snapshot.config.placeId = "123";
  });
  await page
    .getByRole("checkbox", { name: "Select PreviewAccount_00001" })
    .check();
  await page.getByRole("button", { name: /^Change launch target/ }).click();
  const panel = page.getByRole("complementary", { name: "Launch" });
  const server = panel.getByRole("region", { name: "Server" });
  // Only strategies that pick a server offer a region.
  await expect(server.getByLabel("Region", { exact: true })).toHaveCount(0);
  await server.getByRole("radio", { name: /Fullest/ }).click();
  const region = server.getByLabel("Region", { exact: true });
  await expect(region).toHaveValue("");
  await region.selectOption({ label: "Europe" });
  await expect
    .poll(async () => (await lastPatch(page))?.serverRegion)
    .toBe("europe");
  await expect(region).toHaveValue("europe");
  await page.screenshot({
    path: "test-results/screenshots/launch-region-1280.png",
  });
  await region.selectOption({ label: "Anywhere" });
  await expect
    .poll(async () => (await lastPatch(page))?.serverRegion)
    .toBe(null);
});
