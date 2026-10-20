import { chromium } from "@playwright/test";
import assert from "node:assert/strict";

// Recovery of this script's own interrupted fixture, never an ordinary Nocta workspace.
const browser = await chromium.connectOverCDP(
  process.env.NOCTA_NATIVE_CDP ?? "http://127.0.0.1:9225",
);
const page = browser
  .contexts()
  .flatMap((context) => context.pages())
  .find(
    (page) =>
      page.url().includes("tauri.localhost") || page.url().includes("5173"),
  );
assert(page);
const command = (name, args = {}) =>
  page.evaluate(
    ({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args),
    { name, args },
  );
const snapshot = await command("get_snapshot");
assert(snapshot.runtime.dataDirectory.includes("native-smoke-data"));
assert(
  snapshot.tasks.every((task) => task.status !== "running"),
  "Stop running tasks before fixture recovery",
);
const fixtureIds = snapshot.accounts
  .filter((account) => /^Feature_\d{3}$/.test(account.username))
  .map((account) => account.id);
await command("account_action", {
  ids: fixtureIds,
  action: "trash",
  folderId: null,
});
await command("account_action", {
  ids: fixtureIds,
  action: "purge",
  folderId: null,
});
for (const folder of snapshot.folders.filter(
  (folder) => folder.name === "Feature fixture",
))
  await command("delete_folder", { id: folder.id });
await command("patch_config", {
  patch: {
    placeId: "",
    inviteUrl: "",
    preLaunchDelaySecs: 5,
    bulkActionDelaySecs: 30,
    checkerDelaySecs: 0,
    logPreferences: {
      level: "all",
      module: "all",
      showTimestamp: true,
      showAccount: true,
      showModule: true,
      showLevel: false,
    },
  },
});
console.log(JSON.stringify({ removedOwnFixtures: fixtureIds.length }));
await browser.close();
