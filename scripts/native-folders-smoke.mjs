// Folders in the real Nocta window (WebView2 in Tauri), driven by real Windows mouse input.
// Playwright's own drag events skip the window's native drag handling, so only this shows that
// dragging works in the app. Start a debug build on an empty data folder first:
//   NOCTA_DATA_DIR=<empty folder named realtest-data> WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9225 target/debug/nocta-desktop.exe
//   node scripts/native-folders-smoke.mjs <Nocta process ID> <screenshot folder>
import { chromium } from "@playwright/test";
import { spawnSync } from "node:child_process";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";

const [, , pid, shots = "test-results/screenshots"] = process.argv;
const MOUSE = fileURLToPath(new URL("./native-mouse.ps1", import.meta.url));
const mouse = (...args) => {
  const result = spawnSync(
    "powershell",
    [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      MOUSE,
      ...args.map(String),
      "-Window",
      pid,
    ],
    { encoding: "utf8" },
  );
  if (result.status !== 0) throw new Error(result.stderr);
};
const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const browser = await chromium.connectOverCDP("http://127.0.0.1:9225");
let page;
for (let i = 0; i < 100 && !page; i++) {
  page = browser
    .contexts()
    .flatMap((c) => c.pages())
    .find((p) => p.url().includes("5173"));
  if (!page) await wait(100);
}
assert(page, "Nocta window");
await page.waitForFunction(
  () => document.querySelector("h1")?.textContent === "Account Manager",
);
const command = (name, args = {}) =>
  page.evaluate(
    ({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args),
    { name, args },
  );
const snapshot = await command("get_snapshot");
assert(
  snapshot.runtime.dataDirectory.includes("realtest-data"),
  "isolated data folder",
);

// Fixtures: an ungrouped folder, a group with two folders, and a few accounts.
for (const [name, group] of [
  ["Spare", null],
  ["Mains", "Farm"],
  ["Alts", "Farm"],
])
  await command("save_folder", {
    id: null,
    input: { name, color: "#a3a3a3", group },
  });
await page.evaluate(async () => {
  const callback = window.__TAURI_INTERNALS__.transformCallback(() => {});
  await window.__TAURI_INTERNALS__.invoke("import_accounts", {
    text: ["Alpha", "Bravo", "Charlie"]
      .map((n) => `${n}:pw:synthetic-cookie`)
      .join("\n"),
    folderId: null,
    updateExisting: false,
    progress: `__CHANNEL__:${callback}`,
  });
});
await page.waitForFunction(
  () => document.querySelectorAll(".table-row").length === 3,
);
await wait(500);

// Calibration: where a physical cursor position lands in the page.
await page.evaluate(() => {
  window.__last = null;
  window.addEventListener(
    "mousemove",
    (e) => (window.__last = [e.clientX, e.clientY]),
    true,
  );
});
const geo = await page.evaluate(() => ({
  x: window.screenX,
  y: window.screenY,
  dpr: devicePixelRatio,
  w: innerWidth,
  h: innerHeight,
}));
const probe = [
  Math.round((geo.x + geo.w / 2) * geo.dpr),
  Math.round((geo.y + geo.h / 2) * geo.dpr),
];
mouse("move", probe[0] - 5, probe[1] - 5);
mouse("move", ...probe);
await wait(200);
const seen = await page.evaluate(() => window.__last);
assert(seen, "the page sees the real mouse");
const offset = [probe[0] / geo.dpr - seen[0], probe[1] / geo.dpr - seen[1]];
console.log(
  "dpr",
  geo.dpr,
  "offset",
  offset.map((v) => v.toFixed(1)).join(","),
);
const toScreen = (x, y) => [
  Math.round((x + offset[0]) * geo.dpr),
  Math.round((y + offset[1]) * geo.dpr),
];
const center = async (locator, dy = 0.5) => {
  const box = await locator.boundingBox();
  assert(box, "element on screen");
  return toScreen(box.x + box.width / 2, box.y + box.height * dy);
};
const rowOf = (name) => page.locator(".folder-row", { hasText: name });

// 1. Hovering a color swatch: the dot grows, gets a ring and the cursor is a pointer.
mouse("right", ...(await center(rowOf("Spare"))));
await page.getByRole("menuitem", { name: "Color" }).waitFor();
const color = await center(page.getByRole("menuitem", { name: "Color" }));
mouse("glide", color[0] - 40, color[1], ...color);
await page.getByRole("menuitem", { name: "#e06c75" }).waitFor();
const swatch = page.getByRole("menuitem", { name: "#e06c75" });
const target = await center(swatch);
mouse("glide", color[0] + 60, color[1], ...target);
await wait(300);
const hovered = await swatch.evaluate((cell) => ({
  highlighted: cell.hasAttribute("data-highlighted"),
  transform: getComputedStyle(cell.querySelector(".color-dot")).transform,
  ring: getComputedStyle(cell.querySelector(".color-dot")).boxShadow,
  cursor: getComputedStyle(cell).cursor,
}));
console.log("hover", JSON.stringify(hovered));
await page.screenshot({ path: `${shots}/real-hover.png` });
assert(hovered.highlighted, "the hovered swatch is highlighted");
assert.notEqual(hovered.transform, "none", "the dot grows");
assert(hovered.ring.includes("3.5px"), "the dot gets a ring");
assert.equal(hovered.cursor, "pointer");
mouse("click", ...target);
await page.waitForFunction(() => !document.querySelector(".menu"));
await wait(300);
let folders = (await command("get_snapshot")).folders;
assert.equal(
  folders.find((f) => f.name === "Spare").color,
  "#e06c75",
  "the clicked color is saved",
);

// 2. Dragging a folder onto a group header moves it into the group.
mouse(
  "drag",
  ...(await center(rowOf("Spare"))),
  ...(await center(page.locator(".folder-group-header", { hasText: "Farm" }))),
);
await wait(800);
folders = (await command("get_snapshot")).folders;
console.log(
  "after group drop",
  JSON.stringify(folders.map((f) => [f.name, f.group, f.sortOrder])),
);
assert.equal(
  folders.find((f) => f.name === "Spare").group,
  "Farm",
  "dropped into the group",
);

// 3. Reordering inside the group: Alts onto the top half of Mains.
mouse(
  "drag",
  ...(await center(rowOf("Alts"))),
  ...(await center(rowOf("Mains"), 0.2)),
);
await wait(800);
folders = (await command("get_snapshot")).folders
  .filter((f) => f.group === "Farm")
  .sort((a, b) => a.sortOrder - b.sortOrder);
console.log("group order", folders.map((f) => f.name).join(","));
assert.equal(folders[0].name, "Alts", "Alts moved before Mains");

// 4. An account dragged from the table onto a folder moves into it.
const account = page.locator(".table-row", { hasText: "Alpha" });
mouse("drag", ...(await center(account)), ...(await center(rowOf("Mains"))));
await wait(800);
const accounts = (await command("get_snapshot")).accounts;
const mains = folders.find((f) => f.name === "Mains").id;
assert.equal(
  accounts.find((a) => a.username === "Alpha").folderId,
  mains,
  "the account moved",
);

// 5. The folder list never scrolls sideways.
assert.equal(
  await page.locator(".folder-list").evaluate((list) => list.scrollLeft),
  0,
);
await page.screenshot({ path: `${shots}/real-after.png` });
console.log("REAL WINDOW: all checks passed");
await browser.close();
