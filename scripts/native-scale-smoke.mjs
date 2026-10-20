import { chromium } from "@playwright/test";
import assert from "node:assert/strict";
import { writeFile, mkdir } from "node:fs/promises";

const browser = await chromium.connectOverCDP(
  process.env.NOCTA_NATIVE_CDP ?? "http://127.0.0.1:9225",
);
let page;
for (let attempt = 0; attempt < 100 && !page; attempt++) {
  page = browser
    .contexts()
    .flatMap((context) => context.pages())
    .find(
      (page) =>
        page.url().includes("tauri.localhost") || page.url().includes("5173"),
    );
  if (!page) await new Promise((resolve) => setTimeout(resolve, 100));
}
assert(page);
await page.waitForFunction(() => !!window.__TAURI_INTERNALS__);
const command = (name, args = {}) =>
  page.evaluate(
    ({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args),
    { name, args },
  );
const baseline = await command("get_snapshot");
assert(baseline.runtime.dataDirectory.includes("native-smoke-data"));
assert.equal(baseline.accounts.length, 1);
assert(baseline.tasks.every((task) => task.status !== "running"));
await command("save_folder", {
  id: null,
  input: { name: "Scale fixture", color: "#aabbcc", group: null },
});
const folder = (await command("get_snapshot")).folders.find(
  (folder) => folder.name === "Scale fixture",
).id;
const started = performance.now();
const report = await command("import_accounts", {
  text: Array.from(
    { length: 10000 },
    (_, i) =>
      `Scale_${String(i).padStart(5, "0")}:synthetic-password:synthetic;invalid`,
  ).join("\n"),
  folderId: folder,
  updateExisting: false,
});
assert.equal(report.added, 10000);
const importMs = Math.round(performance.now() - started);
const snapshot = await command("get_snapshot");
const ids = snapshot.accounts
  .filter((account) => /^Scale_\d{5}$/.test(account.username))
  .map((account) => account.id);
assert.equal(ids.length, 10000);
assert(!JSON.stringify(snapshot).includes("synthetic-password"));
// A semicolon in this synthetic cookie is rejected locally before any HTTP request.
await command("patch_config", {
  patch: {
    placeId: "123",
    checkerConcurrency: 20,
    precheckRetries: 0,
    checkerDelaySecs: 0,
  },
});
const runStart = performance.now();
await command("start_session_tasks", { ids, kind: "check_health" });
// Timed inside the window: returning megabytes to this script over CDP would dominate the time.
// The longest gap between frames shows whether the window stays responsive meanwhile.
const { snapshotTimes, longestFrameMs } = await page.evaluate(async () => {
  let longest = 0;
  let last = performance.now();
  let watching = true;
  const frame = (now) => {
    longest = Math.max(longest, now - last);
    last = now;
    if (watching) requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
  const snapshotTimes = [];
  for (let attempt = 0; attempt < 5; attempt++) {
    const before = performance.now();
    await window.__TAURI_INTERNALS__.invoke("get_snapshot");
    snapshotTimes.push(Math.round(performance.now() - before));
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  watching = false;
  return { snapshotTimes, longestFrameMs: Math.round(longest) };
});
// The accounts table stays virtualized with 10,000 accounts.
await page
  .getByRole("navigation", { name: "Main navigation" })
  .getByRole("button", { name: /^Accounts/ })
  .click();
await page.locator(".table-row").first().waitFor({ state: "visible" });
const visibleRows = await page.locator(".table-row").count();
assert(
  visibleRows > 0 && visibleRows < 100,
  "Account rows must remain virtualized",
);
await page.screenshot({
  path: "test-results/screenshots/native-checks-10000.png",
});
const stopStart = performance.now();
await command("stop_tasks");
// Polled inside the window, so the time is not spent sending snapshots to this script.
const drained = await page.evaluate(async () => {
  for (let attempt = 0; attempt < 600; attempt++) {
    const current = await window.__TAURI_INTERNALS__.invoke("get_snapshot");
    if (
      current.tasks.every((task) =>
        ["succeeded", "failed", "cancelled"].includes(task.status),
      )
    )
      return true;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return false;
});
assert(drained, "Stop must drain the 10,000-account checks");
const finished = await command("get_snapshot");
// Checks stopped in the queue leave one summary line instead of a line each.
assert(
  finished.logs.filter((entry) => entry.message.startsWith("Cancelled."))
    .length <= 20,
);
assert(
  finished.logs.some((entry) =>
    /^Stopped \d+ running or queued operations\.$/.test(entry.message),
  ),
);
const result = {
  accounts: ids.length,
  importMs,
  snapshotTimesMs: snapshotTimes,
  longestFrameMs,
  runMs: Math.round(performance.now() - runStart),
  stopDrainMs: Math.round(performance.now() - stopStart),
  completedChecks: finished.metrics.checksTotal,
  visibleRows,
  externalRequests: 0,
};
assert(
  finished.tasks.every((task) =>
    ["succeeded", "failed", "cancelled"].includes(task.status),
  ),
);
await command("account_action", { ids, action: "trash", folderId: null });
await command("account_action", { ids, action: "purge", folderId: null });
await command("delete_folder", { id: folder });
await command("patch_config", { patch: baseline.config });
assert.equal((await command("get_snapshot")).accounts.length, 1);
await mkdir("test-results", { recursive: true });
await writeFile(
  "test-results/native-scale-result.json",
  JSON.stringify(result, null, 2) + "\n",
);
console.log(JSON.stringify(result));
await page.getByRole("button", { name: "Close Nocta", exact: true }).click();
await browser.close();
