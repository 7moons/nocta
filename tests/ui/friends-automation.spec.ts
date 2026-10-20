import { expect, test } from "@playwright/test";
import type { FriendEntry } from "../../src/generated/contracts";
import { calls, icon, lastArgs, mockDesktop } from "./desktopFixture";

const friend = (
  id: string,
  name: string,
  kind: number,
  extra: Partial<FriendEntry["presence"]> = {},
): FriendEntry => ({
  user: { id, name, displayName: name.replace("_", " ") },
  presence: {
    userId: id,
    kind,
    placeId: null,
    jobId: null,
    location: null,
    ...extra,
  },
  avatarUrl: icon(id.padStart(6, "a").slice(0, 6)),
});

const signedIn = (snapshot: {
  accounts: { userId: string | null; hasCookie: boolean }[];
}) =>
  snapshot.accounts.forEach((account, index) => {
    account.userId = String(500 + index);
    account.hasCookie = true;
  });

test("friends of an account show where they are and joining one follows them", async ({
  page,
}) => {
  await mockDesktop(page, signedIn, {
    friends: [
      friend("11", "Speed_Runner", 2, { placeId: "91", location: "Doors" }),
      friend("12", "Quiet_Builder", 3, { location: "Studio" }),
      friend("13", "Web_Browser", 1),
      friend("14", "Sleepy_Head", 0),
    ],
  });
  await page.getByRole("button", { name: /^Friends/ }).click();
  await expect(page.getByText("4 friends · 1 in a game")).toBeVisible();
  const list = page.getByRole("list", { name: "Friends" });
  await expect(list.getByRole("listitem")).toHaveCount(4);
  await expect(list.getByRole("listitem").first()).toContainText(
    "In game · Doors",
  );
  await expect(
    page.getByRole("button", { name: "Join Sleepy Head" }),
  ).toBeDisabled();
  expect(await lastArgs(page, "friends_of")).toEqual({ accountId: 1 });
  await page.screenshot({
    path: "test-results/screenshots/friends-1280.png",
  });

  await page.getByRole("button", { name: "Join Speed Runner" }).click();
  await expect
    .poll(() => lastArgs(page, "launch_game"))
    .toEqual({
      ids: [1],
      target: { kind: "follow", username: "Speed_Runner" },
    });

  // Another account lists its own friends; filters narrow the list.
  await page
    .getByLabel("Account")
    .selectOption({ label: "PreviewAccount_00002" });
  await expect
    .poll(() => lastArgs(page, "friends_of"))
    .toEqual({ accountId: 2 });
  await page.getByRole("checkbox", { name: "Only in a game" }).check();
  await expect(list.getByRole("listitem")).toHaveCount(1);
  await page.getByRole("checkbox", { name: "Only in a game" }).uncheck();
  await page.getByLabel("Search friends").fill("quiet");
  await expect(list.getByRole("listitem")).toHaveCount(1);
  await expect(list).toContainText("Quiet Builder");
});

test("presets launch a set of accounts and schedules run them on chosen days", async ({
  page,
}) => {
  await mockDesktop(page);
  await page.getByRole("button", { name: /^Automation/ }).click();
  await expect(page.getByText("No presets yet.")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "New schedule" }),
  ).toBeDisabled();

  // A preset: two accounts, squad into a game, AFK.
  await page.getByRole("button", { name: "New preset" }).click();
  const dialog = page.getByRole("dialog", { name: "New preset" });
  await dialog.getByLabel("Name").fill("Morning farm");
  await dialog.getByRole("radio", { name: "Server by strategy" }).click();
  await dialog.getByLabel("Place ID").fill("1818");
  await dialog.getByLabel("Strategy").selectOption("squad");
  await dialog.getByRole("checkbox", { name: "PreviewAccount_00001" }).check();
  await dialog.getByRole("checkbox", { name: "PreviewAccount_00003" }).check();
  await dialog.getByRole("checkbox", { name: /AFK profile/ }).check();
  await expect(dialog).toContainText("Accounts (2 selected)");
  await dialog.screenshot({
    path: "test-results/screenshots/automation-preset-dialog.png",
  });
  await dialog.getByRole("button", { name: "Save" }).click();
  const presets = (await lastArgs(page, "patch_config"))?.patch
    .launchPresets as { id: string }[];
  expect(presets).toEqual([
    {
      id: expect.any(String),
      name: "Morning farm",
      accountIds: [1, 3],
      target: { kind: "pick", placeId: "1818", strategy: "squad" },
      afk: true,
      hotkey: "",
    },
  ]);
  const row = page.getByRole("list", { name: "Presets" }).getByRole("listitem");
  await expect(row).toContainText("2 accounts · Game 1818 · Squad");
  await expect(row).toContainText("AFK");

  await page.getByRole("button", { name: "Run Morning farm" }).click();
  await expect
    .poll(() => lastArgs(page, "run_preset"))
    .toEqual({ id: presets[0].id });
  await expect(page.getByText("Launching Morning farm…")).toBeVisible();

  // A schedule on weekdays at 07:30 that closes the clients after two hours.
  await page.getByRole("button", { name: "New schedule" }).click();
  const schedule = page.getByRole("dialog", { name: "New schedule" });
  await schedule.getByLabel("Time").fill("07:30");
  await schedule.getByLabel("Close after (minutes)").fill("120");
  await schedule.getByRole("button", { name: "Sat" }).click();
  await schedule.getByRole("button", { name: "Save" }).click();
  expect((await lastArgs(page, "patch_config"))?.patch.schedules).toEqual([
    {
      id: expect.any(String),
      presetId: presets[0].id,
      days: [0, 1, 2, 3, 4, 5],
      time: "07:30",
      closeAfterMinutes: 120,
      enabled: true,
    },
  ]);
  const scheduled = page
    .getByRole("list", { name: "Schedules" })
    .getByRole("listitem");
  await expect(scheduled).toContainText("7:30 AM · Morning farm");
  await expect(scheduled).toContainText("closes after 120 min");
  await expect(scheduled).toContainText("next");
  await page.screenshot({
    path: "test-results/screenshots/automation-1280.png",
  });

  await scheduled.getByRole("switch", { name: "Enabled" }).click();
  await expect
    .poll(
      async () =>
        (await lastArgs(page, "patch_config"))?.patch.schedules?.[0]?.enabled,
    )
    .toBe(false);

  // Deleting the preset removes its schedules too.
  await page.getByRole("button", { name: "Delete Morning farm" }).click();
  await expect
    .poll(async () => (await lastArgs(page, "patch_config"))?.patch)
    .toEqual({ launchPresets: [], schedules: [] });
});

test("Ctrl+K opens the command palette for views, presets, accounts and actions", async ({
  page,
}) => {
  await mockDesktop(page, (snapshot) => {
    snapshot.config.launchPresets = [
      {
        id: "p1",
        name: "Evening grind",
        accountIds: [2],
        target: { kind: "place", placeId: "5" },
        afk: false,
      },
    ];
  });
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await expect(palette).toBeVisible();
  await page.keyboard.type("friends");
  await page.screenshot({
    path: "test-results/screenshots/command-palette-1280.png",
  });
  await page.keyboard.press("Enter");
  await expect(palette).toBeHidden();
  await expect(page.getByRole("heading", { name: "Friends" })).toBeVisible();

  await page.keyboard.press("Control+k");
  await page.keyboard.type("evening");
  await page.keyboard.press("Enter");
  await expect.poll(() => lastArgs(page, "run_preset")).toEqual({ id: "p1" });

  await page.keyboard.press("Control+k");
  await page.keyboard.type("afk");
  await page.keyboard.press("Enter");
  await expect
    .poll(async () => (await lastArgs(page, "patch_config"))?.patch)
    .toEqual({ afkMode: true });

  // Typing an account name opens it for editing; arrows move the selection.
  await page.keyboard.press("Control+k");
  await page.keyboard.type("00002");
  await expect(palette.getByRole("option")).toHaveCount(1);
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("dialog", { name: "Edit account" }),
  ).toContainText("PreviewAccount_00002");
  await page.keyboard.press("Escape");

  await page.keyboard.press("Control+k");
  await page.keyboard.press("ArrowDown");
  await expect(palette.getByRole("option").nth(1)).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await page.keyboard.press("Escape");
  await expect(palette).toBeHidden();
  expect(await calls(page, "close_all_roblox")).toHaveLength(0);
});

test("the new views and join options are translated to Polish", async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem("nocta.language.v1", "pl"),
  );
  await mockDesktop(page, (snapshot) => {
    snapshot.config.placeId = "123";
    snapshot.config.launchPresets = [
      {
        id: "p1",
        name: "Wieczorny grind",
        accountIds: [1, 2],
        target: { kind: "pick", placeId: "5", strategy: "lowest_ping" },
        afk: true,
      },
    ];
    snapshot.config.schedules = [
      {
        id: "s1",
        presetId: "p1",
        days: [5, 6],
        time: "20:00",
        closeAfterMinutes: 90,
        enabled: true,
      },
    ];
  });
  await page.getByRole("button", { name: /^Automatyzacja/ }).click();
  const presets = page.getByRole("list", { name: "Presety" });
  await expect(presets).toContainText("2 konta · Gra 5 · Najniższy ping");
  await expect(page.getByRole("list", { name: "Harmonogramy" })).toContainText(
    "Weekendy · zamyka po 90 min",
  );
  await page.screenshot({
    path: "test-results/screenshots/automation-pl-1280.png",
  });

  await page.getByRole("button", { name: /^Konta/ }).click();
  await page.getByRole("button", { name: /^Zmień cel uruchamiania/ }).click();
  const server = page.getByRole("region", { name: "Serwer" });
  for (const name of [
    "Drużyna",
    "Najniższy ping",
    "Zmiana serwera",
    "Śledź gracza",
  ])
    await expect(
      server.getByRole("radio", { name: new RegExp(name) }),
    ).toBeVisible();
  // Let the panel finish sliding in before the screenshot.
  await page
    .locator(".launch-sheet")
    .evaluate((sheet) =>
      Promise.all(
        sheet.getAnimations({ subtree: true }).map((item) => item.finished),
      ),
    );
  await page.screenshot({
    path: "test-results/screenshots/join-modes-pl-1280.png",
  });
});
