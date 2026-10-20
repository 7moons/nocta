import { expect, test } from "@playwright/test";

test("settings detect the old Nocta database and Roblox Account Manager files and import them", async ({
  page,
}) => {
  await page.goto("/");
  const snapshot = await page.evaluate(async () => {
    const module = await import("/src/lib/preview.ts");
    return module.previewSnapshot();
  });
  snapshot.runtime.platform = "windows";
  await page.addInitScript((initial) => {
    const state = structuredClone(initial);
    const callbacks = new Map();
    let next = 0;
    window.isTauri = true;
    window.fixtureCalls = [];
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    window.__TAURI_INTERNALS__ = {
      metadata: {
        currentWindow: { label: "main" },
        currentWebview: { label: "main" },
      },
      transformCallback(callback) {
        const id = ++next;
        callbacks.set(id, callback);
        return id;
      },
      unregisterCallback(id) {
        callbacks.delete(id);
      },
      async invoke(command, args = {}) {
        window.fixtureCalls.push({ command, args });
        if (command === "plugin:event|listen") return args.handler;
        if (command === "plugin:event|unlisten") return;
        if (command === "get_snapshot") return structuredClone(state);
        if (command === "legacy_detect")
          return [
            {
              path: "C:\\Users\\Me\\AppData\\Documents\\old-nocta.sqlite",
              app: "Nocta (old version)",
              accounts: 5192,
              inTrash: 11,
              folders: 10,
              modifiedAt: "2026-09-30T12:00:00Z",
              needsPassword: false,
            },
            {
              path: "C:\\Users\\Me\\Desktop\\RAM\\AccountData.json",
              app: "Roblox Account Manager (ic3w0lf)",
              accounts: 0,
              inTrash: 0,
              folders: 0,
              modifiedAt: "2026-08-01T12:00:00Z",
              needsPassword: true,
            },
          ];
        if (command === "legacy_import" && args.path.endsWith(".json")) {
          if (args.password !== "hunter2") throw "The password is not correct.";
          return { added: 40, updated: 0, skipped: 0, issues: [] };
        }
        if (command === "legacy_import") {
          // Progress arrives on the channel before the final report.
          callbacks.get(args.progress.id)?.({
            index: 0,
            message: {
              processed: 2600,
              total: 5193,
              added: 2600,
              updated: 0,
              failed: 0,
              committed: false,
            },
          });
          return {
            added: 5192,
            updated: 0,
            skipped: 1,
            issues: [{ line: 812, reason: "OldAlt: No cookie saved." }],
          };
        }
        throw new Error(`Unexpected fixture command: ${command}`);
      },
    };
  }, snapshot);
  await page.reload();

  await page.keyboard.press("Control+4");
  await page
    .getByRole("button", { name: "Import from other apps", exact: true })
    .click();
  const sources = page
    .getByRole("list", { name: "Account files found" })
    .getByRole("listitem");
  const source = sources.first();
  const manager = sources.nth(1);
  await expect(source).toContainText("Nocta (old version)");
  await expect(source).toContainText(
    "5,192 accounts · 10 folders · 11 in Trash",
  );
  // The account manager file is protected with its password, which the import asks for.
  await expect(manager).toContainText("Roblox Account Manager (ic3w0lf)");
  await expect(manager).toContainText(
    "Protected with the account manager's password",
  );
  const importManager = manager.getByRole("button", { name: "Import" });
  await expect(importManager).toBeDisabled();
  await manager
    .getByLabel("Password for Roblox Account Manager (ic3w0lf)")
    .fill("wrong");
  await page.locator("#settings-legacy").scrollIntoViewIfNeeded();
  await page.screenshot({
    path: "test-results/screenshots/legacy-import-1280.png",
  });
  page.once("dialog", (confirm) => void confirm.accept());
  await importManager.click();
  await expect(page.getByRole("alert")).toContainText(
    "The password is not correct.",
  );
  await manager
    .getByLabel("Password for Roblox Account Manager (ic3w0lf)")
    .fill("hunter2");
  page.once("dialog", (confirm) => {
    expect(confirm.message()).toContain("Roblox Account Manager (ic3w0lf)");
    void confirm.accept();
  });
  await importManager.click();
  await expect(
    page.getByText("Imported from Roblox Account Manager (ic3w0lf)"),
  ).toBeVisible();
  await expect(page.getByText("40 accounts added.")).toBeVisible();
  await page.getByRole("button", { name: "Done" }).click();

  await page.getByLabel(/Also import accounts from the old Trash/).check();
  page.once("dialog", (confirm) => {
    expect(confirm.message()).toContain("5,192 accounts and 10 folders");
    void confirm.accept();
  });
  await source.getByRole("button", { name: "Import" }).click();
  await expect(
    page.getByText("Imported from Nocta (old version)"),
  ).toBeVisible();
  await expect(page.getByText("5,192 accounts added, 1 skipped")).toBeVisible();
  await expect(
    page.getByRole("list", { name: "Skipped accounts" }),
  ).toContainText("OldAlt: No cookie saved.");
  const call = await page.evaluate(() =>
    window.fixtureCalls.findLast((item) => item.command === "legacy_import"),
  );
  expect(call.args.includeTrash).toBe(true);
  expect(call.args.path).toContain("old-nocta.sqlite");
});
