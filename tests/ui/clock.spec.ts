import { expect, test } from "@playwright/test";
import { mockDesktop } from "./desktopFixture";

test("times switch between 12- and 24-hour and the choice is remembered", async ({
  page,
}) => {
  await mockDesktop(page, (snapshot) => {
    snapshot.logs = [
      {
        id: 1,
        // Local time, so the expected hour does not depend on the time zone.
        timestamp: new Date(2026, 9, 18, 14, 5, 9).toISOString(),
        level: "INFO",
        module: "SYSTEM",
        message: "Clock fixture",
        accountId: null,
      },
    ];
    snapshot.config.launchPresets = [
      {
        id: "p1",
        name: "Evening",
        accountIds: [1],
        target: { kind: "place", placeId: "5" },
        afk: false,
      },
    ];
    snapshot.config.schedules = [
      {
        id: "s1",
        presetId: "p1",
        days: [0],
        time: "19:30",
        closeAfterMinutes: 0,
        enabled: true,
      },
    ];
  });
  await page.getByRole("button", { name: /^Logs/ }).click();
  const logTime = page.locator(".log-line time").first();
  const lastChecked = page.locator(".table-row").first();
  const schedules = page.getByRole("list", { name: "Schedules" });
  const showSchedules = () =>
    page.getByRole("button", { name: /^Automation/ }).click();
  const choose = async (label: string) => {
    await page.getByRole("button", { name: "Settings", exact: true }).click();
    await page.getByLabel("Time format").selectOption({ label });
  };

  // English writes 12-hour times on its own.
  await expect(logTime).toHaveText("2:05:09 PM");
  await expect(lastChecked).toContainText("02:00 PM");
  await showSchedules();
  await expect(schedules).toContainText("7:30 PM · Evening");

  await choose("24-hour (14:30)");
  await expect(logTime).toHaveText("14:05:09");
  await page.getByRole("button", { name: /^Accounts/ }).click();
  await expect(lastChecked).toContainText("14:00");
  await expect(lastChecked).not.toContainText("PM");
  await showSchedules();
  await expect(schedules).toContainText("19:30 · Evening");
  await page.screenshot({ path: "test-results/screenshots/clock-24h.png" });

  // The choice survives a restart and wins over the language.
  await page.reload();
  await expect(logTime).toHaveText("14:05:09");

  await choose("12-hour (2:30 PM)");
  await expect(logTime).toHaveText("2:05:09 PM");
  await page.getByLabel("Language").selectOption("pl");
  await expect(page.getByLabel("Format godziny")).toHaveValue("12");
  await expect(logTime).toHaveText(/^0?2:05:09\sPM$/);

  await page.getByLabel("Format godziny").selectOption("auto");
  await expect(logTime).toHaveText("14:05:09");
  await page.screenshot({ path: "test-results/screenshots/clock-setting.png" });
  await page.getByLabel("Język").selectOption("en");
});
