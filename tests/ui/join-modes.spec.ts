import { expect, test } from "@playwright/test";
import { calls, lastArgs, mockDesktop } from "./desktopFixture";

test("join strategies and following a player launch with the matching target", async ({
  page,
}) => {
  await mockDesktop(page, (snapshot) => {
    snapshot.config.placeId = "123";
  });
  const lastPatch = async () =>
    (await lastArgs(page, "patch_config"))?.patch as Record<string, unknown>;
  const targetButton = page.getByRole("button", {
    name: /^Change launch target/,
  });
  await page
    .getByRole("checkbox", { name: "Select PreviewAccount_00001" })
    .check();
  await targetButton.click();
  const panel = page.getByRole("complementary", { name: "Launch" });
  const server = panel.getByRole("region", { name: "Server" });

  // Squad: the fullest server with room for every account, chosen when launching.
  await server.getByRole("radio", { name: /Squad/ }).click();
  await expect.poll(lastPatch).toEqual({
    joinStrategy: "squad",
    inviteUrl: "",
    followUsername: "",
  });
  await expect(server.getByRole("radio", { name: /Squad/ })).toHaveAttribute(
    "aria-checked",
    "true",
  );
  await expect(panel.locator(".launch-footer-summary")).toHaveText(
    "Game 123 · Squad",
  );
  await page.screenshot({
    path: "test-results/screenshots/join-squad-1280.png",
  });
  await panel.getByRole("button", { name: "Launch 1 account" }).click();
  await expect
    .poll(async () => (await calls(page, "launch_game")).length)
    .toBe(1);
  expect((await lastArgs(page, "launch_game"))?.target).toEqual({
    kind: "pick",
    placeId: "123",
    strategy: "squad",
    excludeJob: null,
  });

  // Lowest ping and server hop work the same way.
  for (const [label, strategy] of [
    ["Lowest ping", "lowest_ping"],
    ["Server hop", "hop"],
  ] as const) {
    await targetButton.click();
    await server.getByRole("radio", { name: new RegExp(label) }).click();
    await expect
      .poll(async () => (await lastPatch())?.joinStrategy)
      .toBe(strategy);
    await expect(targetButton).toContainText(label);
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: /^Launch selected/ }).click();
    await expect
      .poll(async () => (await lastArgs(page, "launch_game"))?.target)
      .toMatchObject({ kind: "pick", strategy });
  }

  // Following a player checks the username, then joins whatever server they are in.
  await targetButton.click();
  await server.getByRole("radio", { name: /Follow a player/ }).click();
  const username = server.getByLabel("Player username");
  await username.fill("nobody");
  await server.getByRole("button", { name: "Follow", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText(
    "No Roblox account is called nobody.",
  );
  await username.fill("builderman");
  await server.getByRole("button", { name: "Follow", exact: true }).click();
  await expect.poll(lastPatch).toEqual({
    followUsername: "builderman",
    joinStrategy: null,
    inviteUrl: "",
  });
  await expect(server).toContainText("Following builderman.");
  await expect(panel.locator(".launch-footer-summary")).toHaveText(
    "Following builderman",
  );
  await page.screenshot({
    path: "test-results/screenshots/join-follow-1280.png",
  });
  await panel.getByRole("button", { name: "Launch 1 account" }).click();
  await expect
    .poll(async () => (await lastArgs(page, "launch_game"))?.target)
    .toEqual({ kind: "follow", username: "builderman" });

  // Any server turns both off again.
  await targetButton.click();
  await server.getByRole("radio", { name: /Any server/ }).click();
  await expect.poll(lastPatch).toEqual({ inviteUrl: "", followUsername: "" });
});

test("launch options switch AFK mode and auto-rejoin", async ({ page }) => {
  await mockDesktop(page, (snapshot) => {
    snapshot.config.placeId = "123";
  });
  await page.getByRole("button", { name: /^Change launch target/ }).click();
  const panel = page.getByRole("complementary", { name: "Launch" });
  await panel.getByRole("button", { name: "Options" }).click();
  await expect(panel.getByText("15 FPS, lowest graphics")).toBeVisible();
  await panel.getByRole("switch", { name: "AFK mode" }).click();
  await expect
    .poll(async () => (await lastArgs(page, "patch_config"))?.patch)
    .toEqual({ afkMode: true });
  await panel.getByRole("switch", { name: "Auto-rejoin" }).click();
  await expect
    .poll(async () => (await lastArgs(page, "patch_config"))?.patch)
    .toEqual({ autoRejoin: true });
  await expect(panel.getByRole("switch", { name: "AFK mode" })).toBeChecked();
  await panel.screenshot({
    path: "test-results/screenshots/launch-options-afk.png",
  });
});
