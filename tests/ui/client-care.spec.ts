import { expect, test, type Page } from "@playwright/test";
import type { GameInstance } from "../../src/generated/contracts";
import { calls, lastArgs, mockDesktop } from "./desktopFixture";

const lastPatch = async (page: Page) =>
  (await lastArgs(page, "patch_config"))?.patch;

const client = (index: number, patch: Partial<GameInstance>): GameInstance => ({
  id: `client-${index}`,
  accountId: index,
  username: `PreviewAccount_0000${index}`,
  placeId: "920587237",
  mode: "public",
  status: "running",
  pid: 5000 + index,
  processName: "RobloxPlayerBeta.exe",
  startedAt: new Date().toISOString(),
  exitedAt: null,
  detail: "Running.",
  stage: "loading",
  disconnect: null,
  ...patch,
});

test("running clients show their stage and why Roblox disconnected them, and windows can be arranged", async ({
  page,
}) => {
  await mockDesktop(page, (snapshot) => {
    snapshot.runtime.capabilities = ["launch", "sessions", "instances"];
    snapshot.instances = [
      client(1, { stage: "in_game" }),
      client(2, { stage: "joining" }),
      client(3, {
        stage: "disconnected",
        disconnect: "277: the connection was lost (DisconnectConnectionLost)",
      }),
    ];
  });
  await page.getByRole("button", { name: /^Instances/ }).click();
  const panel = page.locator(".instances-view");
  await expect(panel).toContainText("in game");
  await expect(panel).toContainText("joining");
  await expect(panel).toContainText(
    "Roblox disconnected it: 277: the connection was lost (DisconnectConnectionLost)",
  );
  await page.screenshot({
    path: "test-results/screenshots/instances-stage-disconnect-1280.png",
  });

  await page.getByRole("button", { name: "Arrange windows" }).click();
  await expect(page.getByText("Arranged 3 windows.")).toBeVisible();
  expect(await calls(page, "arrange_windows")).toHaveLength(1);

  // The same action from the command palette.
  await page.keyboard.press("Control+k");
  await page.keyboard.type("grid");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await expect(palette.getByRole("option")).toContainText([
    "Arrange Roblox windows in a grid",
  ]);
  await page.screenshot({
    path: "test-results/screenshots/palette-arrange-1280.png",
  });
  await page.keyboard.press("Enter");
  await expect
    .poll(async () => (await calls(page, "arrange_windows")).length)
    .toBe(2);
});

test("client care and per-account profiles are saved from Settings", async ({
  page,
}) => {
  await mockDesktop(page);
  await page.getByRole("button", { name: /^Settings/ }).click();
  await page
    .getByRole("button", { name: /Game launch/ })
    .first()
    .click();

  const clients = page.locator("#settings-clients");
  await clients.scrollIntoViewIfNeeded();
  await clients.getByRole("switch", { name: "Wait until in game" }).click();
  await expect.poll(() => lastPatch(page)).toEqual({ launchPacing: "fixed" });
  await clients.getByLabel("Longest wait to join").fill("90");
  await clients.getByLabel("Longest wait to join").blur();
  await expect.poll(() => lastPatch(page)).toEqual({ inGameTimeoutSecs: 90 });
  await clients
    .getByLabel("Monitors for arranging", { exact: true })
    .selectOption("primary");
  await expect
    .poll(() => lastPatch(page))
    .toEqual({ arrangeMonitors: "primary" });
  for (const [name, key, value] of [
    ["Name client windows", "nameWindows", false],
    ["Arrange automatically", "autoArrange", true],
    ["Close frozen clients", "closeHung", false],
    ["Close disconnected clients", "closeDisconnected", false],
    ["Keep the computer awake", "keepAwake", false],
  ] as const) {
    await clients.getByRole("switch", { name }).click();
    await expect.poll(() => lastPatch(page)).toEqual({ [key]: value });
  }
  await clients.getByLabel("Restart other accounts every").fill("60");
  await clients.getByLabel("Restart other accounts every").blur();
  await expect.poll(() => lastPatch(page)).toEqual({ restartAltsMinutes: 60 });
  await clients.screenshot({
    path: "test-results/screenshots/settings-client-care-1280.png",
  });

  const profiles = page.locator("#settings-profiles");
  await profiles.scrollIntoViewIfNeeded();
  await profiles
    .getByLabel("Other accounts: Priority")
    .selectOption("below_normal");
  await expect
    .poll(async () => (await lastPatch(page))?.altProfile?.priority)
    .toBe("below_normal");
  await profiles
    .getByRole("checkbox", { name: "Other accounts: Efficiency mode (EcoQoS)" })
    .check();
  await profiles.getByLabel("Other accounts: CPU limit (0 = none)").fill("25");
  await profiles.getByLabel("Other accounts: CPU limit (0 = none)").blur();
  await profiles.getByLabel("Other accounts: width").fill("800");
  await profiles.getByLabel("Other accounts: width").blur();
  await expect
    .poll(async () => (await lastPatch(page))?.altProfile)
    .toEqual({
      priority: "below_normal",
      ecoMode: true,
      cpuLimitPercent: 25,
      windowWidth: 800,
      windowHeight: 600,
      lowMemoryPriority: false,
      trimEverySecs: 0,
      ramCapMb: 0,
    });
  await profiles.getByLabel("Main accounts: Priority").selectOption("normal");

  // One click sets other accounts up for many clients; main accounts stay as they were.
  await profiles
    .getByRole("button", { name: "Optimize other accounts for many instances" })
    .click();
  await expect
    .poll(() => lastPatch(page))
    .toEqual({
      altProfile: {
        priority: "below_normal",
        ecoMode: true,
        cpuLimitPercent: 25,
        windowWidth: 800,
        windowHeight: 600,
        lowMemoryPriority: true,
        trimEverySecs: 60,
        ramCapMb: 400,
      },
      clientLowGraphics: true,
    });
  await expect(
    profiles.getByRole("checkbox", {
      name: "Other accounts: Low memory priority",
    }),
  ).toBeChecked();
  await expect(
    profiles.getByLabel("Other accounts: Give memory back every (0 = off)"),
  ).toHaveValue("60");
  await expect(
    profiles.getByLabel("Other accounts: RAM cap per client (0 = none)"),
  ).toHaveValue("400");

  // Any window size, down to a single pixel.
  await profiles.getByLabel("Other accounts: width").fill("1");
  await profiles.getByLabel("Other accounts: width").blur();
  await profiles.getByLabel("Other accounts: height").fill("1");
  await profiles.getByLabel("Other accounts: height").blur();
  await expect
    .poll(async () => {
      const alt = (await lastPatch(page))?.altProfile;
      return alt && [alt.windowWidth, alt.windowHeight];
    })
    .toEqual([1, 1]);
  await expect(profiles).toContainText("Any size works, down to 1×1");
  await profiles.screenshot({
    path: "test-results/screenshots/settings-client-profiles-1280.png",
  });
});

test("fullest and emptiest servers, in-game pacing and main accounts", async ({
  page,
}) => {
  await mockDesktop(page, (snapshot) => {
    snapshot.config.placeId = "123";
  });
  await page
    .getByRole("checkbox", { name: "Select PreviewAccount_00001" })
    .check();
  const targetButton = page.getByRole("button", {
    name: /^Change launch target/,
  });
  const panel = page.getByRole("complementary", { name: "Launch" });
  const server = panel.getByRole("region", { name: "Server" });
  for (const [label, strategy] of [
    ["Fullest", "fullest"],
    ["Emptiest", "emptiest"],
  ] as const) {
    await targetButton.click();
    await server.getByRole("radio", { name: new RegExp(label) }).click();
    await expect
      .poll(async () => (await lastPatch(page))?.joinStrategy)
      .toBe(strategy);
    if (strategy === "fullest")
      await page.screenshot({
        path: "test-results/screenshots/join-fullest-1280.png",
      });
    await panel.getByRole("button", { name: "Launch 1 account" }).click();
    await expect
      .poll(async () => (await lastArgs(page, "launch_game"))?.target)
      .toEqual({
        kind: "pick",
        placeId: "123",
        strategy,
        excludeJob: null,
      });
  }

  // Launch options: wait for each account to join before starting the next.
  await targetButton.click();
  await panel.getByRole("button", { name: "Options" }).click();
  const pacing = panel.getByRole("switch", { name: "Wait until in game" });
  await expect(pacing).toBeChecked();
  await pacing.click();
  await expect.poll(() => lastPatch(page)).toEqual({ launchPacing: "fixed" });
  await page.keyboard.press("Escape");

  // Marking a main account.
  await page.locator(".table-row").nth(1).dblclick();
  const dialog = page.getByRole("dialog", { name: "Edit account" });
  await dialog.getByRole("checkbox", { name: /Main account/ }).check();
  await dialog.screenshot({
    path: "test-results/screenshots/account-main.png",
  });
  await dialog.getByRole("button", { name: "Save changes" }).click();
  await expect(dialog).toBeHidden();
  expect((await lastArgs(page, "update_account_details"))?.details).toEqual({
    alias: "",
    note: "",
    color: null,
    pinned: false,
    isMain: true,
    fields: [],
    afkPoint: null,
  });
});
