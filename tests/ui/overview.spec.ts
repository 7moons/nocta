import { expect, test } from "@playwright/test";

test("overview shows what needs attention and leads to the filtered accounts", async ({
  page,
}) => {
  await page.goto("/?benchmark=40");
  await page.getByRole("button", { name: /^Overview/ }).click();
  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();

  const tiles = page.locator(".overview-tiles");
  await expect(tiles).toContainText("40");
  await expect(
    page.getByRole("region", { name: "Session health" }),
  ).toBeVisible();
  const setup = page.getByRole("region", { name: "Setup" });
  await expect(setup).toContainText(/of \d+ ready/);
  await expect(setup).toContainText("Accounts");
  await page.screenshot({ path: "test-results/screenshots/overview-1280.png" });

  // "Need attention" opens the accounts filtered to the first problem state.
  await tiles.getByRole("button", { name: /Need attention/ }).click();
  await expect(
    page.getByRole("heading", { name: "Account Manager" }),
  ).toBeVisible();
  const filter = page.getByLabel("Filter by session status");
  await expect(filter).toHaveValue("captcha_required");
  const rows = page.getByRole("table").getByRole("row");
  await expect(rows.nth(1)).toContainText("Captcha required");

  // Back returns to the overview; the filter belongs to that history entry.
  await page.getByRole("button", { name: "Go back" }).click();
  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();

  // A state in the legend filters to that state.
  await page
    .getByRole("region", { name: "Session health" })
    .getByRole("button", { name: /^Healthy/ })
    .click();
  await expect(filter).toHaveValue("healthy");

  // Choosing a folder in the sidebar clears the state filter.
  await page.getByRole("button", { name: /^All accounts/ }).click();
  await expect(filter).toHaveValue("all");
});

test("overview puts the accounts in a game first, with their game, stage and time", async ({
  page,
}) => {
  const { mockDesktop, calls, lastArgs } = await import("./desktopFixture");
  const minutesAgo = (minutes: number) =>
    new Date(Date.now() - minutes * 60_000).toISOString();
  await mockDesktop(
    page,
    (snapshot) => {
      snapshot.runtime.capabilities = ["launch", "sessions", "instances"];
      snapshot.runtime.browserReady = true;
      snapshot.config.placeId = "920587237";
      snapshot.accounts.forEach((account, index) => {
        account.robux = (index + 1) * 250;
        account.robuxCheckedAt = new Date().toISOString();
        account.presence = {
          userPresenceType: 1,
          lastLocation: "Website",
          lastOnline: null,
          placeId: null,
        };
      });
      snapshot.accounts[1].alias = "Farm";
      // Account 5 plays on its own, outside Nocta.
      snapshot.accounts[4].presence = {
        userPresenceType: 2,
        lastLocation: "Adopt Me!",
        lastOnline: null,
        placeId: null,
      };
      const client = (
        id: number,
        stage: "in_game" | "joining" | "disconnected",
        minutes: number,
      ) => ({
        id: `client-${id}`,
        accountId: id,
        username: `PreviewAccount_0000${id}`,
        placeId: "920587237",
        mode: "public" as const,
        status: "running" as const,
        pid: 5000 + id,
        processName: "RobloxPlayerBeta.exe",
        startedAt: minutesAgo(minutes),
        exitedAt: null,
        detail: "Running.",
        stage,
        disconnect: null,
        memoryMb: id === 1 ? 1450 : 820,
      });
      snapshot.instances = [
        client(1, "in_game", 135),
        client(2, "in_game", 42),
        client(3, "joining", 1),
        client(4, "disconnected", 18),
      ];
      snapshot.config.antiAfk = true;
      snapshot.config.autoRejoin = true;
      snapshot.config.launchPresets = [
        {
          id: "p1",
          name: "Night farm",
          accountIds: [1, 2],
          target: { kind: "place", placeId: "920587237" },
          afk: true,
          hotkey: "",
        },
      ];
      snapshot.config.schedules = [
        {
          id: "s1",
          presetId: "p1",
          days: [0, 1, 2, 3, 4, 5, 6],
          time: "23:30",
          closeAfterMinutes: 0,
          enabled: true,
        },
      ];
    },
    {},
    6,
  );
  await page.getByRole("button", { name: /^Overview/ }).click();
  const tiles = page.locator(".overview-tiles");
  await expect(tiles.getByRole("button", { name: /In a game/ })).toContainText(
    "3",
  );
  await expect(tiles).toContainText("6 of 6 accounts online");
  await expect(
    tiles.getByRole("button", { name: /Roblox clients/ }),
  ).toContainText("1 disconnected");
  await expect(tiles.getByRole("button", { name: /Robux/ })).toContainText(
    "5,250",
  );

  const playing = page.getByRole("region", { name: "Playing now" });
  const rows = playing.getByRole("listitem");
  await expect(rows).toHaveCount(5);
  // Problems first, then who is in a game, then who is still joining.
  await expect(rows.nth(0)).toContainText("PreviewAccount_00004");
  await expect(rows.nth(0)).toContainText("disconnected");
  await expect(rows.nth(1)).toContainText("PreviewAccount_00001");
  await expect(rows.nth(1)).toContainText("Game 920587237");
  await expect(rows.nth(1)).toContainText("2 h 15 min");
  await expect(rows.nth(1)).toContainText("1.4 GB");
  await expect(rows.nth(0)).toContainText("820 MB");
  await expect(rows.nth(2)).toContainText("Farm");
  await expect(rows.nth(2)).toContainText("0 h 42 min");
  await expect(rows.nth(3)).toContainText("Adopt Me!");
  await expect(rows.nth(4)).toContainText("joining");
  await expect(playing).toContainText(
    "3 in a game · 1 joining · 1 disconnected",
  );
  const next = page.getByRole("region", { name: "Right now" });
  await expect(next).toContainText("Night farm ·");
  await expect(next).toContainText("Anti-AFK · Auto-rejoin");
  // The computer's memory: commit near its limit is what stops more clients from starting.
  await expect(next).toContainText(
    "RAM 23.8 GB / 31.1 GB · reserved 41.7 GB / 48.0 GB",
  );
  await expect(
    next.getByRole("meter", { name: "Reserved memory" }),
  ).toHaveAttribute("aria-valuenow", "87");
  await expect(next).toContainText("Make the page file larger");
  // Setup is complete here, so it is not shown.
  await expect(page.getByRole("region", { name: "Setup" })).toHaveCount(0);
  await page.screenshot({
    path: "test-results/screenshots/overview-playing-1280.png",
  });

  await playing.getByRole("button", { name: "Arrange windows" }).click();
  await expect
    .poll(async () => (await calls(page, "arrange_windows")).length)
    .toBe(1);
  await rows
    .nth(0)
    .getByRole("button", { name: /Close .*PreviewAccount_00004/ })
    .click();
  await expect
    .poll(() => lastArgs(page, "close_instance"))
    .toEqual({ id: "client-4" });
  await expect(rows).toHaveCount(4);
});

test("the overview reads well in Polish", async ({ page }) => {
  const { mockDesktop } = await import("./desktopFixture");
  await mockDesktop(page, (snapshot) => {
    snapshot.runtime.capabilities = ["launch", "sessions", "instances"];
    snapshot.instances = [1, 2].map((id) => ({
      id: `client-${id}`,
      accountId: id,
      username: `PreviewAccount_0000${id}`,
      placeId: "920587237",
      mode: "public" as const,
      status: "running" as const,
      pid: 5000 + id,
      processName: "RobloxPlayerBeta.exe",
      startedAt: new Date(Date.now() - id * 3_600_000).toISOString(),
      exitedAt: null,
      detail: "Running.",
      stage: "in_game" as const,
      disconnect: null,
    }));
  });
  await page.evaluate(() => localStorage.setItem("nocta.language.v1", "pl"));
  await page.reload();
  await page.getByRole("button", { name: /^Przegląd/ }).click();
  const playing = page.getByRole("region", { name: "W grze teraz" });
  await expect(playing.getByRole("listitem")).toHaveCount(2);
  await expect(page.locator(".overview-tiles")).toContainText("W grze");
  // Accounts with a running client count as online even before presence is refreshed.
  await expect(page.locator(".overview-tiles")).toContainText(
    "2 z 3 kont online",
  );
  await page.screenshot({
    path: "test-results/screenshots/overview-pl-1280.png",
  });
});
