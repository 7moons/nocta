import { expect, test } from "@playwright/test";

test("the interface language switches to Polish and is remembered", async ({
  page,
}) => {
  await page.goto("/?benchmark=5");
  await expect(
    page.getByRole("heading", { name: "Account Manager" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByLabel("Language").selectOption("pl");

  // Everything re-renders in Polish at once, including the navigation.
  await expect(page.getByRole("heading", { name: "Ustawienia" })).toBeVisible();
  const accountsNav = page
    .getByRole("navigation", { name: "Nawigacja główna" })
    .getByRole("button", { name: /^Konta/ });
  await expect(accountsNav).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.lang)).toBe("pl");

  // Polish plural forms: 5 → "kont", 2 → "konta", 1 → "konto".
  await accountsNav.click();
  await expect(page.locator(".view-header")).toContainText("5 kont");
  for (const name of ["PreviewAccount_00001", "PreviewAccount_00002"])
    await page.getByRole("checkbox", { name: `Zaznacz ${name}` }).check();
  await expect(
    page.getByRole("button", { name: /^Uruchom zaznaczone/ }),
  ).toContainText("2");
  await expect(page.locator(".view-header")).toContainText("Zaznaczono: 2");
  await page
    .getByRole("checkbox", { name: "Zaznacz PreviewAccount_00002" })
    .uncheck();
  await page.getByRole("button", { name: /^Zmień cel uruchamiania/ }).click();
  await expect(
    page.getByRole("button", { name: "Uruchom 1 konto" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await page.screenshot({ path: "test-results/screenshots/accounts-pl.png" });

  // The choice survives a restart.
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Menedżer kont" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Ustawienia", exact: true }).click();
  await page.getByLabel("Język").selectOption("en");
  await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
});
