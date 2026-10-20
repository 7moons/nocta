import { expect, test } from "@playwright/test";
import { mockDesktop } from "./desktopFixture";

type Tree = { [key: string]: string | Tree };

test("every language keeps the placeholders and plural forms of English", async ({
  page,
}) => {
  await page.goto("/");
  const problems = await page.evaluate(async () => {
    const { LANGUAGES } = await import("/src/i18n/index.ts");
    const en = (await LANGUAGES[0].load()) as unknown as Tree;
    const placeholders = (text: string) =>
      [...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort();
    const isPlural = (node: unknown) =>
      !!node && typeof node === "object" && "other" in node;
    const found: string[] = [];
    for (const language of LANGUAGES) {
      if (language.id === "en") continue;
      const walk = (source: Tree, target: Tree, path: string) => {
        for (const [key, value] of Object.entries(source)) {
          const here = `${language.id}:${path}${key}`;
          const other = target?.[key];
          if (other === undefined) {
            found.push(`${here} missing`);
          } else if (typeof value === "string" || isPlural(value)) {
            const english = placeholders(
              typeof value === "string" ? value : String((value as Tree).other),
            ).join();
            const forms =
              typeof other === "string"
                ? [other]
                : Object.values(other as Tree).map(String);
            if (isPlural(value) && !isPlural(other))
              found.push(`${here} lost its plural forms`);
            for (const form of forms) {
              const own = placeholders(form).join();
              // A plural form may leave out {count} ("Account copied.").
              const withoutCount = placeholders(
                typeof value === "string"
                  ? value
                  : String((value as Tree).other),
              )
                .filter((name) => name !== "count")
                .join();
              if (own !== english && !(isPlural(value) && own === withoutCount))
                found.push(`${here} has {${own}} instead of {${english}}`);
            }
            if (typeof other === "string" && !other.trim())
              found.push(`${here} is empty`);
          } else {
            walk(value as Tree, other as Tree, `${path}${key}.`);
          }
        }
      };
      walk(en, (await language.load()) as unknown as Tree, "");
    }
    return found;
  });
  expect(problems).toEqual([]);
});

const LANGUAGES = [
  { id: "de", name: "Deutsch", accounts: "Konten", title: "Kontoverwaltung" },
  {
    id: "es",
    name: "Español",
    accounts: "Cuentas",
    title: "Gestor de cuentas",
  },
  {
    id: "fr",
    name: "Français",
    accounts: "Comptes",
    title: "Gestionnaire de comptes",
  },
  {
    id: "pt-BR",
    name: "Português (Brasil)",
    accounts: "Contas",
    title: "Gerenciador de contas",
  },
  { id: "tr", name: "Türkçe", accounts: "Hesaplar", title: "Hesap Yöneticisi" },
  {
    id: "ru",
    name: "Русский",
    accounts: "Аккаунты",
    title: "Менеджер аккаунтов",
  },
];

test("each language can be picked in Settings and translates the whole app", async ({
  page,
}) => {
  await mockDesktop(page, () => {}, {}, 5);
  for (const language of LANGUAGES) {
    await page.keyboard.press("Control+4");
    // The language list, whatever language its label is in.
    await page
      .locator('select:has(option[value="pt-BR"])')
      .selectOption(language.id);
    // The language's strings load when it is chosen.
    await expect
      .poll(() => page.evaluate(() => document.documentElement.lang))
      .toBe(language.id);
    await page.keyboard.press("Control+1");
    await expect(
      page.getByRole("heading", { name: language.title }),
    ).toBeVisible();
    await expect(
      page.locator(".nav-item", { hasText: language.accounts }).first(),
    ).toBeVisible();
    // No placeholder or key leaks into the page.
    const text = await page.locator("body").innerText();
    expect(text).not.toMatch(/\{\w+\}/);
    expect(text).not.toMatch(/\b(accounts|launch|settings)\.[a-z]+/);
    await page.screenshot({
      path: `test-results/screenshots/language-${language.id}-1280.png`,
    });
    // The densest screens, in the languages with the longest words.
    if (language.id === "de" || language.id === "fr") {
      await page.keyboard.press("Control+4");
      await page.screenshot({
        path: `test-results/screenshots/language-${language.id}-settings-1280.png`,
      });
      await page.keyboard.press("Control+1");
    }
    if (language.id === "ru" || language.id === "pt-BR") {
      await page.locator(".view-header .header-actions button").nth(2).click();
      await page.locator(".launch-sheet").waitFor();
      await page.waitForTimeout(400);
      await page.screenshot({
        path: `test-results/screenshots/language-${language.id}-launch-1280.png`,
      });
      await page.keyboard.press("Escape");
    }
  }
});

test("Russian picks the right plural form for 1, 2 and 5 accounts", async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem("nocta.language.v1", "ru"),
  );
  await mockDesktop(page, () => {}, {}, 5);
  await expect(page.locator(".view-header")).toContainText("5 аккаунтов");
  for (const [count, form] of [
    [1, "Запустить 1 аккаунт"],
    [2, "Запустить 2 аккаунта"],
    [5, "Запустить 5 аккаунтов"],
  ] as const) {
    const text = await page.evaluate(
      async ([count]) => {
        const { translate } = await import("/src/i18n/index.ts");
        return translate("ru", "launch.launchCount", { count });
      },
      [count],
    );
    expect(text).toBe(form);
  }
});

test("the compact list fits more accounts on screen and is remembered", async ({
  page,
}) => {
  await mockDesktop(page, () => {}, {}, 60);
  const rowHeight = () =>
    page
      .locator(".table-row")
      .first()
      .evaluate((row) => row.offsetHeight);
  const visibleRows = () =>
    page.locator(".accounts-table").evaluate((table) => {
      const scroller = table.querySelector(".virtual-space")?.parentElement;
      const rows = [...table.querySelectorAll<HTMLElement>(".table-row")];
      const bottom = scroller!.getBoundingClientRect().bottom;
      return rows.filter((row) => row.getBoundingClientRect().bottom <= bottom)
        .length;
    });
  expect(await rowHeight()).toBe(48);
  const before = await visibleRows();

  // Turned on from the command palette; the setting is saved like any other.
  await page.keyboard.press("Control+k");
  await page.keyboard.type("compact");
  await page.keyboard.press("Enter");
  await expect.poll(rowHeight).toBe(32);
  const after = await visibleRows();
  expect(after).toBeGreaterThan(before);
  await expect(
    page.locator(".table-row .account-name small").first(),
  ).toBeHidden();
  await page.screenshot({
    path: "test-results/screenshots/accounts-compact-1280.png",
  });

  // And off again from Settings → Appearance.
  await page.keyboard.press("Control+4");
  await page
    .getByRole("button", { name: /^Appearance/ })
    .first()
    .click();
  await page.getByRole("switch", { name: "Compact account list" }).click();
  await page.keyboard.press("Control+1");
  await expect.poll(rowHeight).toBe(48);
  console.log(`rows on screen: normal ${before}, compact ${after}`);
});
