import { expect, test, type Page } from "@playwright/test";
import type { AppSnapshot } from "../../src/generated/contracts";
import { lastArgs, mockDesktop } from "./desktopFixture";

const FOLDERS = [
  { id: 1, name: "Mains", color: "#e06c75", group: "Farm", sortOrder: 0 },
  { id: 2, name: "Alts", color: "#98c379", group: "Farm", sortOrder: 1 },
  { id: 3, name: "Spare", color: "#a3a3a3", group: null, sortOrder: 0 },
];

function withFolders(snapshot: AppSnapshot) {
  snapshot.folders = structuredClone(FOLDERS);
  snapshot.accounts[0].folderId = 1;
  snapshot.accounts[1].folderId = 2;
}

const row = (page: Page, name: string) =>
  page.locator(".folder-row", { hasText: name });
const panel = (page: Page) => page.locator(".folder-panel");

test("folders are created and renamed in place, without a dialog", async ({
  page,
}) => {
  await mockDesktop(page, withFolders);
  await expect(
    panel(page).getByRole("button", { name: /^Farm/ }),
  ).toContainText("2");
  await expect(row(page, "Mains")).toContainText("1");

  await page.getByRole("button", { name: "New folder" }).click();
  await page.getByRole("textbox", { name: "New folder" }).fill("Night");
  await page.keyboard.press("Enter");
  expect(await lastArgs(page, "save_folder")).toEqual({
    id: null,
    input: { name: "Night", color: "#a3a3a3", group: null },
  });

  await row(page, "Spare").locator(".folder-select").dblclick();
  await page.getByRole("textbox", { name: "Rename" }).fill("Reserve");
  await page.keyboard.press("Enter");
  // Shown at once, before the backend answers.
  await expect(row(page, "Reserve")).toBeVisible();
  expect(await lastArgs(page, "save_folder")).toEqual({
    id: 3,
    input: { name: "Reserve", color: "#a3a3a3", group: null },
  });
  // Escape leaves the name as it was.
  await row(page, "Mains").locator(".folder-select").focus();
  await page.keyboard.press("F2");
  await page.getByRole("textbox", { name: "Rename" }).fill("Gone");
  await page.keyboard.press("Escape");
  await expect(row(page, "Mains")).toBeVisible();
  await page.screenshot({ path: "test-results/screenshots/folders-1280.png" });
});

test("the folder menu sets the color, moves to a group and deletes", async ({
  page,
}) => {
  await mockDesktop(page, withFolders);
  await row(page, "Mains").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Color" }).hover();
  const swatch = page.getByRole("menuitem", { name: "#61afef" });
  // Each swatch shows its color, not an empty cell.
  const dot = await swatch.locator("span").first().boundingBox();
  expect(dot?.width).toBeGreaterThan(10);
  await page.screenshot({
    path: "test-results/screenshots/folders-colors-1280.png",
  });
  await swatch.click();
  expect(await lastArgs(page, "save_folder")).toEqual({
    id: 1,
    input: { name: "Mains", color: "#61afef", group: "Farm" },
  });

  await row(page, "Spare").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Move to group" }).hover();
  await page.screenshot({
    path: "test-results/screenshots/folders-menu-1280.png",
  });
  await page.getByRole("menuitem", { name: "Farm", exact: true }).click();
  expect(await lastArgs(page, "move_folder")).toEqual({
    id: 3,
    group: "Farm",
    before: null,
  });
  // Spare is now the last folder of the group.
  await expect(
    page.locator(".folder-group .folder-row .folder-name"),
  ).toHaveText(["Mains", "Alts", "Spare"]);

  // The group's buttons show on hover, as for the user.
  await page.locator(".folder-group-header", { hasText: "Farm" }).hover();
  await page.getByRole("button", { name: "Actions for group Farm" }).click();
  await page.getByRole("menuitem", { name: "Rename group" }).click();
  await page.getByRole("textbox", { name: "Rename group" }).fill("Farm EU");
  await page.keyboard.press("Enter");
  expect(await lastArgs(page, "rename_folder_group")).toEqual({
    from: "Farm",
    to: "Farm EU",
  });
  await expect(
    panel(page).getByRole("button", { name: /^Farm EU/ }),
  ).toBeVisible();

  page.once("dialog", (dialog) => void dialog.accept());
  await row(page, "Alts").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Delete" }).click();
  expect(await lastArgs(page, "delete_folder")).toEqual({ id: 2 });
  await expect(row(page, "Alts")).toHaveCount(0);
});

test("folders are reordered and accounts moved by dragging", async ({
  page,
}) => {
  await mockDesktop(page, withFolders);
  // Onto the top half of Mains: before it.
  await row(page, "Alts").dragTo(row(page, "Mains"), {
    targetPosition: { x: 40, y: 4 },
  });
  expect(await lastArgs(page, "move_folder")).toEqual({
    id: 2,
    group: "Farm",
    before: 1,
  });
  await expect(
    page.locator(".folder-group .folder-row .folder-name"),
  ).toHaveText(["Alts", "Mains"]);

  const first = page.locator(".table-row").first();
  await first.dragTo(row(page, "Spare"));
  const moved = await lastArgs(page, "account_action");
  expect(moved?.action).toBe("move");
  expect(moved?.folderId).toBe(3);
  expect(moved?.ids).toHaveLength(1);
});

test("any color comes from the palette, the hue strip or a hex code", async ({
  page,
}) => {
  await mockDesktop(page, withFolders);
  await row(page, "Spare").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Color" }).hover();
  await page.getByRole("menuitem", { name: "Custom color…" }).click();
  const dialog = page.getByRole("dialog", { name: "Folder color" });
  await expect(dialog).toBeVisible();
  const area = dialog.getByRole("slider", {
    name: "Saturation and brightness",
  });
  const hue = dialog.getByRole("slider", { name: "Hue" });
  const hex = dialog.getByRole("textbox", { name: "Hex code" });
  const box = await area.boundingBox();
  expect(box!.width).toBeGreaterThan(200);
  expect(box!.height).toBeGreaterThan(120);
  expect((await hue.boundingBox())!.width).toBeGreaterThan(200);

  // Top right of the square: the hue at full strength (red to start with), give or take
  // the last pixel.
  await area.click({ position: { x: box!.width - 1, y: 1 } });
  await expect(hex).toHaveValue(/^#f[ef]0[01]0[01]$/);
  // Further along the hue strip: blue.
  await hue.fill("240");
  await expect(hex).toHaveValue(/^#0[01]0[01]f[ef]$/);
  // Bottom of the square is black whatever the hue.
  await area.click({ position: { x: box!.width / 2, y: box!.height - 1 } });
  await expect(hex).toHaveValue(/^#0[0-3]0[0-3]0[0-9a-f]$/);

  await hex.fill("#123abc");
  await expect(dialog.locator(".folder-color-preview svg")).toHaveCSS(
    "color",
    "rgb(18, 58, 188)",
  );
  await page.screenshot({
    path: "test-results/screenshots/folders-custom-color-1280.png",
  });
  await dialog.getByRole("button", { name: "Save color" }).click();
  await expect(dialog).toBeHidden();
  expect(await lastArgs(page, "save_folder")).toEqual({
    id: 3,
    input: { name: "Spare", color: "#123abc", group: null },
  });
  await expect(row(page, "Spare").locator("svg").first()).toHaveCSS(
    "color",
    "rgb(18, 58, 188)",
  );
});

test("color swatches are centered, the chosen one has a check instead of a frame", async ({
  page,
}) => {
  await mockDesktop(page, withFolders);
  await row(page, "Mains").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Color" }).hover();
  const chosen = page.getByRole("menuitem", { name: "#e06c75" });
  await expect(chosen).toHaveAttribute("aria-checked", "true");
  await expect(chosen.locator("svg")).toHaveCount(1);
  await expect(
    page.getByRole("menuitem", { name: "#98c379" }).locator("svg"),
  ).toHaveCount(0);
  expect(
    await chosen.evaluate((cell) => getComputedStyle(cell).boxShadow),
  ).toBe("none");
  for (const name of ["#e06c75", "#98c379", "#c678dd"]) {
    const cell = page.getByRole("menuitem", { name });
    const outer = (await cell.boundingBox())!;
    const dot = (await cell.locator(".color-dot").boundingBox())!;
    expect(
      Math.abs(outer.x + outer.width / 2 - (dot.x + dot.width / 2)),
    ).toBeLessThanOrEqual(1);
    expect(
      Math.abs(outer.y + outer.height / 2 - (dot.y + dot.height / 2)),
    ).toBeLessThanOrEqual(1);
  }
  const submenu = page.locator(".menu").filter({ has: chosen });
  await submenu.screenshot({
    path: "test-results/screenshots/folders-swatches.png",
    scale: "device",
  });
});

test("a picked color is saved to the palette and used from the menu", async ({
  page,
}) => {
  await mockDesktop(page, withFolders);
  await row(page, "Spare").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Color" }).hover();
  await page.getByRole("menuitem", { name: "Custom color…" }).click();
  const dialog = page.getByRole("dialog", { name: "Folder color" });
  await dialog.getByRole("textbox", { name: "Hex code" }).fill("#123abc");
  await dialog.getByRole("button", { name: "Add to palette" }).click();
  expect(await lastArgs(page, "patch_config")).toEqual({
    patch: { folderColors: ["#123abc"] },
  });
  const yours = dialog.getByRole("group", { name: "Your colors" });
  await expect(yours.getByRole("button", { name: "#123abc" })).toBeVisible();
  // Already in the palette: it cannot be added twice.
  await expect(
    dialog.getByRole("button", { name: "Add to palette" }),
  ).toBeDisabled();
  await dialog.getByRole("textbox", { name: "Hex code" }).fill("#ffaa00");
  await dialog.getByRole("button", { name: "Add to palette" }).click();
  await expect(yours.getByRole("button", { name: "#ffaa00" })).toBeVisible();
  await page.screenshot({
    path: "test-results/screenshots/folders-palette-1280.png",
  });
  await yours.getByRole("button", { name: "#123abc" }).hover();
  await yours
    .getByRole("button", { name: "Remove #123abc from the palette" })
    .click();
  expect(await lastArgs(page, "patch_config")).toEqual({
    patch: { folderColors: ["#ffaa00"] },
  });
  await page.keyboard.press("Escape");

  // The saved color is offered in the menu.
  await row(page, "Mains").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Color" }).hover();
  const saved = page
    .getByRole("group", { name: "Your colors" })
    .getByRole("menuitem", { name: "#ffaa00" });
  await saved.click();
  expect(await lastArgs(page, "save_folder")).toEqual({
    id: 1,
    input: { name: "Mains", color: "#ffaa00", group: "Farm" },
  });
});

test("closing a dialog opened from a menu leaves the app clickable", async ({
  page,
}) => {
  await mockDesktop(page, withFolders);
  await row(page, "Spare").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Color" }).hover();
  await page.getByRole("menuitem", { name: "Custom color…" }).click();
  await expect(
    page.getByRole("dialog", { name: "Folder color" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect
    .poll(() => page.evaluate(() => document.body.style.pointerEvents))
    .toBe("");
  await row(page, "Mains").locator(".folder-select").click({ timeout: 2000 });
  await expect(row(page, "Mains")).toHaveClass(/active/);
});

test("the folder panel with groups and colored folders, as the README shows it", async ({
  page,
}) => {
  await mockDesktop(
    page,
    (snapshot) => {
      snapshot.folders = [
        {
          id: 1,
          name: "Mains",
          color: "#e06c75",
          group: "Farm EU",
          sortOrder: 0,
        },
        {
          id: 2,
          name: "Alts",
          color: "#98c379",
          group: "Farm EU",
          sortOrder: 1,
        },
        {
          id: 3,
          name: "Traders",
          color: "#61afef",
          group: "Farm US",
          sortOrder: 0,
        },
        { id: 4, name: "Fresh", color: "#e5c07b", group: null, sortOrder: 0 },
        { id: 5, name: "Spare", color: "#c678dd", group: null, sortOrder: 1 },
      ];
      snapshot.accounts.forEach((account, index) => {
        account.folderId = [1, 1, 1, 2, 2, 2, 2, 3, 3, 4, null, 5][index % 12];
      });
    },
    {},
    48,
  );
  await expect(page.locator(".folder-group")).toHaveCount(2);
  await page.screenshot({
    path: "test-results/screenshots/readme-accounts-1280.png",
  });
});
