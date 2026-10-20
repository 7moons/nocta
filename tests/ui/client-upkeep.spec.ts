import { expect, test, type Page } from "@playwright/test";
import { calls, lastArgs, mockDesktop } from "./desktopFixture";

const lastPatch = async (page: Page) =>
  (await lastArgs(page, "patch_config"))?.patch;

test("anti-AFK, memory, tray, autostart and shortcuts are set in Settings", async ({
  page,
}) => {
  await mockDesktop(page);
  await page.getByRole("button", { name: /^Settings/ }).click();
  await page
    .getByRole("button", { name: /Game launch/ })
    .first()
    .click();

  const afk = page.locator("#settings-anti-afk");
  await afk.getByRole("switch", { name: "Keep clients active" }).click();
  await expect.poll(() => lastPatch(page)).toEqual({ antiAfk: true });
  await afk.getByLabel("Press a key every").fill("12");
  await afk.getByLabel("Press a key every").blur();
  await expect.poll(() => lastPatch(page)).toEqual({ antiAfkMinutes: 12 });
  await afk.getByLabel("Key", { exact: true }).selectOption("jump");
  await expect.poll(() => lastPatch(page)).toEqual({ antiAfkAction: "jump" });

  const performance = page.locator("#settings-performance");
  await performance.getByRole("switch", { name: "Trim client memory" }).click();
  await expect.poll(() => lastPatch(page)).toEqual({ trimMemory: true });
  await performance.getByLabel("Memory limit per client").fill("2048");
  await performance.getByLabel("Memory limit per client").blur();
  await expect.poll(() => lastPatch(page)).toEqual({ memoryLimitMb: 2048 });
  await afk.scrollIntoViewIfNeeded();
  await page.screenshot({
    path: "test-results/screenshots/settings-anti-afk-1280.png",
  });

  // System and shortcuts live under General.
  await page
    .getByRole("button", { name: /^General/ })
    .first()
    .click();
  const system = page.locator("#settings-system");
  await expect(
    system.getByRole("switch", { name: "Start in the tray" }),
  ).toBeDisabled();
  await system
    .getByRole("switch", { name: "Keep running in the tray" })
    .click();
  await expect.poll(() => lastPatch(page)).toEqual({ closeToTray: true });
  await system.getByRole("switch", { name: "Start with Windows" }).click();
  await expect.poll(() => lastPatch(page)).toEqual({ startWithWindows: true });
  await expect(
    system.getByRole("switch", { name: "Start in the tray" }),
  ).toBeEnabled();

  const hotkeys = page.locator("#settings-hotkeys");
  const show = hotkeys.getByLabel("Show Nocta", { exact: true });
  await show.click();
  await expect(show).toHaveAttribute("placeholder", "Press the keys…");
  // A plain letter would fire while typing anywhere, so it is not taken.
  await page.keyboard.press("n");
  await expect(show).toBeFocused();
  await page.keyboard.press("Control+Alt+N");
  await expect
    .poll(() => lastPatch(page))
    .toEqual({
      hotkeys: { show: "Ctrl+Alt+N", closeRoblox: "", toggleAfk: "" },
    });
  await expect(show).toHaveValue("Ctrl+Alt+N");
  await hotkeys.getByLabel("Close all Roblox", { exact: true }).click();
  await page.keyboard.press("F9");
  await expect
    .poll(async () => (await lastPatch(page))?.hotkeys?.closeRoblox)
    .toBe("F9");
  await hotkeys.scrollIntoViewIfNeeded();
  await page.screenshot({
    path: "test-results/screenshots/settings-system-hotkeys-1280.png",
  });
  await hotkeys.getByRole("button", { name: "Clear Show Nocta" }).click();
  await expect
    .poll(async () => (await lastPatch(page))?.hotkeys?.show)
    .toBe("");
});

test("FastFlags offer only the flags Roblox allows, with matching values", async ({
  page,
}) => {
  await mockDesktop(page);
  await page.getByRole("button", { name: /^Settings/ }).click();
  await page
    .getByRole("button", { name: /Roblox client/ })
    .first()
    .click();
  const flags = page.locator("#settings-client-flags");
  await expect(flags).toContainText("No FastFlags set.");
  const add = flags.getByRole("button", { name: "Add" });
  await flags
    .getByLabel("Flag", { exact: true })
    .selectOption("FIntDebugForceMSAASamples");
  await flags.getByLabel("Value", { exact: true }).fill("lots");
  await expect(add).toBeDisabled();
  await flags.getByLabel("Value", { exact: true }).fill("4");
  await add.click();
  await expect
    .poll(() => lastPatch(page))
    .toEqual({
      fastFlags: [{ name: "FIntDebugForceMSAASamples", value: "4" }],
    });
  await flags
    .getByLabel("Flag", { exact: true })
    .selectOption("DFFlagDisableDPIScale");
  await expect(flags.getByLabel("Value", { exact: true })).toHaveValue("True");
  await add.click();
  await expect(flags.getByRole("list")).toContainText("DFFlagDisableDPIScale");
  // A flag that is already set is not offered again.
  await expect(
    flags.getByLabel("Flag", { exact: true }).locator("option", {
      hasText: "FIntDebugForceMSAASamples",
    }),
  ).toHaveCount(0);
  await flags.scrollIntoViewIfNeeded();
  await page.screenshot({
    path: "test-results/screenshots/settings-fast-flags-1280.png",
  });
  await flags
    .getByRole("button", { name: "Remove FIntDebugForceMSAASamples" })
    .click();
  await expect
    .poll(() => lastPatch(page))
    .toEqual({
      fastFlags: [{ name: "DFFlagDisableDPIScale", value: "True" }],
    });
});

test("Robux, playtime and one-click session fixes on the accounts page", async ({
  page,
}) => {
  await mockDesktop(page, (snapshot) => {
    snapshot.accounts.forEach((account, index) => {
      account.userId = String(500 + index);
      account.hasCookie = true;
      account.hasPassword = true;
      account.checkedAt = new Date().toISOString();
    });
    snapshot.accounts[0].health = "invalid_session";
    snapshot.accounts[1].checkedAt = "2026-08-01T12:00:00Z";
    snapshot.accounts[2].playtimeSecs = 3 * 3600 + 25 * 60;
  });

  const banner = page.locator(".session-fix");
  await expect(banner).toContainText(
    "1 session expired · 1 session not checked for 14 days",
  );
  await page.screenshot({
    path: "test-results/screenshots/accounts-fix-sessions-1280.png",
  });
  await banner.getByRole("button", { name: "Fix" }).click();
  await expect
    .poll(async () => (await calls(page, "start_session_tasks")).length)
    .toBe(2);
  const tasks = (await calls(page, "start_session_tasks")).map(
    (call) => call.args,
  );
  expect(tasks).toEqual([
    { ids: [1], kind: "refresh_cookie" },
    { ids: [2], kind: "check_health" },
  ]);
  await expect(banner).toHaveCount(0);

  await page.getByRole("checkbox", { name: "Select all" }).check();
  await page
    .getByLabel("Run account operation")
    .selectOption({ label: "Check Robux" });
  await expect(page.getByText("Robux checked for 3 accounts.")).toBeVisible();
  expect(await lastArgs(page, "refresh_robux")).toEqual({ ids: [1, 2, 3] });

  await page
    .getByRole("button", { name: "Actions for PreviewAccount_00003" })
    .click();
  const facts = page.locator(".account-facts");
  await expect(facts).toContainText("Robux3,000");
  await expect(facts).toContainText("Playtime3 h 25 min");
  await page.getByRole("dialog").screenshot({
    path: "test-results/screenshots/account-robux-playtime.png",
  });
});

test("a preset gets its own shortcut", async ({ page }) => {
  await mockDesktop(page);
  await page.getByRole("button", { name: /^Automation/ }).click();
  await page.getByRole("button", { name: "New preset" }).click();
  const dialog = page.getByRole("dialog", { name: "New preset" });
  await dialog.getByLabel("Name").fill("Night");
  await dialog.getByLabel("Place ID").fill("1818");
  await dialog.getByRole("checkbox", { name: "PreviewAccount_00001" }).check();
  await dialog.getByLabel("Shortcut", { exact: true }).click();
  await page.keyboard.press("Control+Shift+1");
  await expect(dialog.getByLabel("Shortcut", { exact: true })).toHaveValue(
    "Ctrl+Shift+1",
  );
  await dialog.getByRole("button", { name: "Save" }).click();
  const presets = (await lastArgs(page, "patch_config"))?.patch
    .launchPresets as { hotkey: string }[];
  expect(presets[0].hotkey).toBe("Ctrl+Shift+1");
});
