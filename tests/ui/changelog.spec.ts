import { expect, test } from "@playwright/test";
import { mockDesktop } from "./desktopFixture";

test("the version in the status bar opens what's new", async ({ page }) => {
  await mockDesktop(page);
  const dialog = page.getByRole("dialog", { name: "What's new" });
  await expect(dialog).toBeHidden();
  await page.getByRole("button", { name: "v1.0.0" }).click();
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("heading", { name: "v1.0.0" })).toBeVisible();
  await expect(dialog).toContainText("The first public release.");
  await expect(
    dialog.getByRole("heading", { name: "Launching" }),
  ).toBeVisible();
  await expect(dialog.locator("code").first()).toHaveText(
    "username:password:cookie",
  );
  await page.screenshot({
    path: "test-results/screenshots/changelog-1280.png",
  });
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();

  await page.keyboard.press("Control+K");
  await page
    .getByRole("dialog", { name: "Command palette" })
    .getByRole("combobox")
    .fill("what's new");
  await page.keyboard.press("Enter");
  await expect(dialog).toBeVisible();
});

test("what's new opens by itself once after an update", async ({ page }) => {
  await mockDesktop(page);
  const dialog = page.getByRole("dialog", { name: "What's new" });
  await expect(dialog).toBeHidden();
  await page.evaluate(() =>
    localStorage.setItem("nocta.changelog.seen", "0.9.0"),
  );
  await page.reload();
  await expect(dialog).toBeVisible();
  await page.keyboard.press("Escape");
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Account Manager" }),
  ).toBeVisible();
  await expect(dialog).toBeHidden();
});
