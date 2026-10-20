import { expect, test } from "@playwright/test";

test("add and update dialogs preview every line before importing", async ({
  page,
}) => {
  await page.goto("/?benchmark=3");
  await page
    .getByRole("button", { name: "Add accounts", exact: true })
    .first()
    .click();
  const dialog = page.getByRole("dialog", { name: "Add accounts" });
  const editor = dialog.getByRole("textbox", { name: "Accounts", exact: true });
  await expect(editor).toBeFocused();
  await editor.fill(
    [
      "NewPlayer:secret:_|WARNING:-DO-NOT-SHARE-THIS.--|cookie",
      "NoCookie:secret:",
      "PreviewAccount_00001:pw:cookie",
      "NewPlayer::cookie-again",
      "",
      "SecondNew::cookie",
    ].join("\n"),
  );
  const check = dialog.getByRole("status");
  await expect(check).toContainText("2 ready");
  await expect(check).toContainText("1 already exist");
  await expect(check).toContainText("1 duplicate");
  await expect(check).toContainText("1 invalid");
  const problems = dialog.getByRole("list", {
    name: "Lines that will be skipped",
  });
  await expect(problems.getByRole("listitem")).toHaveCount(3);
  await expect(problems).toContainText("The cookie is missing.");
  // Secrets never appear in the preview.
  await expect(problems).not.toContainText("secret");
  await expect(
    dialog.getByRole("button", { name: "Import 2 accounts" }),
  ).toBeEnabled();
  await page.screenshot({
    path: "test-results/screenshots/import-preview-1280.png",
  });

  await dialog.getByRole("button", { name: "Remove problem lines" }).click();
  await expect(editor).toHaveValue(
    [
      "NewPlayer:secret:_|WARNING:-DO-NOT-SHARE-THIS.--|cookie",
      "",
      "SecondNew::cookie",
    ].join("\n"),
  );
  await expect(problems).toHaveCount(0);
  await page.keyboard.press("Escape");

  await page
    .getByRole("button", { name: "Update accounts", exact: true })
    .click();
  const update = page.getByRole("dialog", { name: "Update accounts" });
  await update
    .getByRole("textbox", { name: "Accounts", exact: true })
    .fill("PreviewAccount_00002:new:cookie\nUnknownPlayer:pw:cookie");
  await expect(update.getByRole("status")).toContainText("1 to update");
  await expect(update.getByRole("status")).toContainText("1 not found");
  await expect(
    update.getByRole("button", { name: "Update 1 account" }),
  ).toBeEnabled();
});
