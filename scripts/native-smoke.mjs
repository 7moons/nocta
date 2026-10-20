import { chromium } from "@playwright/test";
import assert from "node:assert/strict";

// Isolated debug NOCTA_DATA_DIR or a copied production portable package; WebView2 loopback CDP.
const browser = await chromium.connectOverCDP(
  process.env.NOCTA_NATIVE_CDP ?? "http://127.0.0.1:9225",
);
const packaged = process.argv.includes("--packaged");
let page;
for (let attempt = 0; attempt < 100 && !page; attempt++) {
  page = browser
    .contexts()
    .flatMap((context) => context.pages())
    .find(
      (page) =>
        page.url().includes("5173") ||
        page.url().includes("tauri.localhost") ||
        page.url().startsWith("tauri://"),
    );
  if (!page) await new Promise((resolve) => setTimeout(resolve, 100));
}
assert(page, "Nocta WebView must be running");
await page.waitForFunction(() => !!window.__TAURI_INTERNALS__);
await page.waitForFunction(
  () => document.querySelector("h1")?.textContent === "Account Manager",
);
const command = (name, args = {}) =>
  page.evaluate(
    ({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args),
    { name, args },
  );
let snapshot = await command("get_snapshot");
assert(
  snapshot.runtime.dataDirectory.includes("native-smoke-data"),
  "Use the isolated smoke workspace",
);
if (packaged && process.env.NOCTA_NATIVE_PACKAGE_ROOT) {
  const root = process.env.NOCTA_NATIVE_PACKAGE_ROOT.replaceAll(
    "\\",
    "/",
  ).toLowerCase();
  assert(snapshot.runtime.portable, "Test the production portable package");
  assert.equal(
    snapshot.runtime.dataDirectory.replaceAll("\\", "/").toLowerCase(),
    `${root}/data`,
  );
  assert(
    snapshot.runtime.browserPath
      ?.replaceAll("\\", "/")
      .toLowerCase()
      .startsWith(`${root}/runtime/`),
    "Chromium must come from the package itself",
  );
}
if (process.argv.includes("--features")) {
  assert.equal(snapshot.accounts.length, 1, "Run the baseline smoke first");
  const originalConfig = snapshot.config;
  // The new panel auto-loads public game metadata. This offline test covers native
  // task/config paths; HTTP metadata/server contracts are exercised by fixtures separately.
  await page.evaluate(() => {
    const nativeInvoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = (name, ...args) => {
      if (["get_place_info", "list_servers"].includes(name)) {
        return Promise.reject({
          message: "Offline native verification: game metadata is disabled.",
        });
      }
      return nativeInvoke(name, ...args);
    };
  });
  const waitFor = async (predicate, message) => {
    for (let attempt = 0; attempt < 200; attempt++) {
      const current = await command("get_snapshot");
      if (predicate(current)) return current;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error(message);
  };
  const failure = (name, args) =>
    page.evaluate(
      async ({ name, args }) => {
        try {
          await window.__TAURI_INTERNALS__.invoke(name, args);
          return null;
        } catch (error) {
          return error;
        }
      },
      { name, args },
    );
  assert(snapshot.runtime.capabilities.includes("launch"));
  assert(snapshot.runtime.capabilities.includes("instances"));
  assert(
    snapshot.config.multiInstance,
    "Multi-instance must default to on on Windows",
  );
  assert(snapshot.launcher.multiInstanceSupported);
  assert(
    ["acquired", "abandoned"].includes(snapshot.launcher.multiInstance),
    "The native helper must own the singleton lock",
  );
  await command("patch_config", { patch: { multiInstance: false } });
  assert.equal(
    (await command("get_snapshot")).launcher.multiInstance,
    "disabled",
  );
  await command("patch_config", { patch: { multiInstance: true } });
  assert(
    ["acquired", "abandoned"].includes(
      (await command("get_snapshot")).launcher.multiInstance,
    ),
  );
  assert.deepEqual(await command("list_instances"), []);
  assert.equal(await command("close_all_owned_instances"), 0);
  await command("save_folder", {
    id: null,
    input: {
      name: "Feature fixture",
      color: "#AABBCC",
      group: "Offline verification",
    },
  });
  const folderId = (await command("get_snapshot")).folders.find(
    (folder) => folder.name === "Feature fixture",
  ).id;
  const imported = await page.evaluate(async (folderId) => {
    const events = [];
    const callback = window.__TAURI_INTERNALS__.transformCallback((packet) => {
      if (packet.message) events.push(packet.message);
    });
    const text =
      Array.from(
        { length: 51 },
        (_, i) =>
          `Feature_${String(i).padStart(3, "0")}:synthetic-password:synthetic-cookie:${i}`,
      ).join("\n") + "\ninvalid\nFeature_000:duplicate:cookie";
    const report = await window.__TAURI_INTERNALS__.invoke("import_accounts", {
      text,
      folderId,
      updateExisting: false,
      progress: `__CHANNEL__:${callback}`,
    });
    // Channel events may be delivered in the next WebView event-loop turn.
    await new Promise((resolve) => setTimeout(resolve, 100));
    window.__TAURI_INTERNALS__.unregisterCallback(callback);
    return { report, events };
  }, folderId);
  assert.equal(imported.report.added, 51);
  assert.equal(imported.report.skipped, 2);
  assert(imported.events.some((event) => !event.committed));
  assert(
    imported.events.some(
      (event) =>
        event.committed && event.added === 51 && event.processed === 53,
    ),
  );
  const fixtures = (await command("get_snapshot")).accounts.filter(
    (account) => account.folderId === folderId,
  );
  const ids = fixtures.map((account) => account.id);
  for (const account of fixtures)
    await command("update_account", {
      id: account.id,
      input: {
        username: account.username,
        folderId,
        password: null,
        cookie: "",
      },
      validateSession: false,
    });
  await command("account_action", {
    ids: [ids[0]],
    action: "trash",
    folderId: null,
  });
  await command("account_action", {
    ids: [ids[0]],
    action: "restore_unfiled",
    folderId: null,
  });
  assert.equal(
    (await command("get_snapshot")).accounts.find(
      (account) => account.id === ids[0],
    ).folderId,
    null,
  );
  await command("account_action", { ids: [ids[0]], action: "move", folderId });
  await command("patch_config", {
    patch: {
      placeId: "",
      inviteUrl: "",
      preLaunchDelaySecs: 30,
      bulkActionDelaySecs: 0,
      checkerDelaySecs: 0,
      logPreferences: {
        level: "ERROR",
        module: "all",
        showTimestamp: false,
        showAccount: true,
        showModule: true,
        showLevel: true,
      },
    },
  });
  assert.equal(
    (
      await command("resolve_launch_target", {
        accountId: null,
        target: { kind: "configured" },
      })
    ).placeId,
    "920587237",
  );
  const privateSummary = await command("resolve_launch_target", {
    accountId: ids[0],
    target: {
      kind: "private",
      placeId: "123",
      accessCode: "synthetic-private-access",
      linkCode: "synthetic-link",
    },
  });
  assert.equal(privateSummary.mode, "private");
  assert(privateSummary.targetId.startsWith("target-"));
  assert(!JSON.stringify(privateSummary).includes("synthetic-"));
  assert.equal(
    (
      await failure("resolve_launch_target", {
        accountId: null,
        target: { kind: "job", placeId: "123", jobId: "" },
      })
    ).code,
    "validation",
  );
  const launchTasks = await command("launch_game", {
    ids: [ids[0], ids[1]],
    target: { kind: "public", placeId: "123" },
  });
  await waitFor(
    (value) =>
      value.tasks.some(
        (task) => task.id === launchTasks[0] && task.status === "running",
      ),
    "First launch must enter the cancellable pre-launch wait",
  );
  const bulkProgress = page.getByRole("status", {
    name: "Bulk account progress",
  });
  await bulkProgress.waitFor();
  await page.waitForFunction(() => {
    const text =
      document.querySelector('[aria-label="Bulk account progress"]')
        ?.textContent ?? "";
    return (
      text.includes("1 active") &&
      text.includes("1 queued") &&
      text.includes("Current: Feature_000") &&
      text.includes("Next: Feature_001")
    );
  });
  await page.screenshot({
    path: "test-results/screenshots/native-bulk-progress.png",
  });
  await command("stop_tasks");
  await waitFor(
    (value) =>
      launchTasks.every((id) =>
        value.tasks.some(
          (task) => task.id === id && task.status === "cancelled",
        ),
      ),
    "Launch cancellation must drain",
  );
  await bulkProgress.waitFor({ state: "hidden" });
  await command("patch_config", { patch: { placeId: "123" } });
  const before = (await command("get_snapshot")).metrics.checksTotal;
  await command("start_session_tasks", { ids, kind: "check_health" });
  await waitFor(
    (value) =>
      value.tasks.filter(
        (task) =>
          ids.includes(task.accountId) &&
          task.kind === "check_health" &&
          ["succeeded", "failed", "cancelled"].includes(task.status),
      ).length === ids.length,
    "Empty cookies must finish their checks",
  );
  const round = await command("get_snapshot");
  assert.equal(round.metrics.checksTotal, before + ids.length);
  for (const account of round.accounts.filter((account) =>
    ids.includes(account.id),
  )) {
    assert.equal(account.lastTask?.kind, "check_health");
    assert.equal(account.lastTask?.status, "succeeded");
    assert(account.lastTask.finishedAt && account.lastTask.durationMs !== null);
  }
  await command("stop_tasks");
  await waitFor(
    (value) =>
      value.tasks.every((task) =>
        ["succeeded", "failed", "cancelled"].includes(task.status),
      ),
    "Stop must drain all work",
  );
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: /^Accounts/ })
    .click();
  await page
    .getByRole("button", { name: "All accounts", exact: false })
    .click();
  await page
    .getByRole("checkbox", {
      name: `Select ${fixtures[0].username}`,
      exact: true,
    })
    .check();
  // "Launch selected" would launch right away, so only the launch panel is opened here.
  await page.getByRole("button", { name: /^Change launch target/ }).click();
  await expectVisible(
    page.getByRole("complementary", { name: "Launch", exact: true }),
  );
  await page.screenshot({
    path: "test-results/screenshots/native-launch-panel.png",
  });
  await page.keyboard.press("Escape");
  await command("account_action", { ids, action: "trash", folderId: null });
  await command("account_action", { ids, action: "purge", folderId: null });
  await command("delete_folder", { id: folderId });
  await command("patch_config", { patch: originalConfig });
  const final = await command("get_snapshot");
  assert.equal(final.accounts.length, 1);
  assert(!JSON.stringify(final).includes("synthetic-"));
  console.log(
    JSON.stringify({
      nativeImportProgress: "passed",
      restoreDestination: "passed",
      nativeLaunchCommandsAndCancellation: "passed",
      nativeBulkCurrentNextAndCounts: "passed",
      nativeMultiInstanceDefaultReadyAndToggle: "passed",
      nativeSessionChecksAndStop: "passed",
      secretRedaction: "passed",
    }),
  );
  await page.getByRole("button", { name: "Close Nocta", exact: true }).click();
  await browser.close();
  process.exit(0);
}
async function expectVisible(locator) {
  await locator.waitFor({ state: "visible", timeout: 10000 });
}
if (process.argv.includes("--sessions")) {
  assert.equal(snapshot.accounts.length, 1);
  const account = snapshot.accounts[0];
  const update = (cookie) =>
    command("update_account", {
      id: account.id,
      input: {
        username: account.username,
        folderId: account.folderId,
        password: null,
        cookie,
      },
    });
  const waitTerminal = async (id) => {
    for (let attempt = 0; attempt < 100; attempt++) {
      const task = (await command("get_snapshot")).tasks.find(
        (task) => task.id === id,
      );
      if (task && ["succeeded", "failed", "cancelled"].includes(task.status))
        return task;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error("Task did not finish");
  };
  await update(""); // Empty synthetic cookie never causes an external HTTP request.
  await command("patch_config", { patch: { checkerDelaySecs: 0 } });
  const [validated] = await command("start_session_tasks", {
    ids: [account.id],
    kind: "validate_cookie",
  });
  assert.equal((await waitTerminal(validated)).status, "succeeded");
  assert.equal((await command("get_snapshot")).accounts[0].cookieValid, false);
  await command("patch_config", { patch: { checkerDelaySecs: 30 } });
  const [cancelled] = await command("start_session_tasks", {
    ids: [account.id],
    kind: "validate_cookie",
  });
  const conflict = await page.evaluate(async ({ id, username, folderId }) => {
    try {
      await window.__TAURI_INTERNALS__.invoke("update_account", {
        id,
        input: {
          username,
          folderId,
          password: null,
          cookie: "other-synthetic-cookie",
        },
      });
      return null;
    } catch (error) {
      return error.code;
    }
  }, account);
  assert.equal(conflict, "busy");
  await command("stop_tasks");
  assert.equal((await waitTerminal(cancelled)).status, "cancelled");
  await command("patch_config", { patch: { checkerDelaySecs: 0 } });
  await update("synthetic-cookie:tail");
  snapshot = await command("get_snapshot");
  assert.equal(snapshot.accounts[0].cookieValid, null);
  assert(!JSON.stringify(snapshot).includes("synthetic-cookie"));
  await page.screenshot({
    path: "test-results/screenshots/native-sessions.png",
  });
  console.log(
    JSON.stringify({
      nativeSessions: "passed",
      accountReservation: "passed",
      cancellation: "passed",
      credentialMetadataReset: "passed",
    }),
  );
  await page.getByRole("button", { name: "Close Nocta", exact: true }).click();
  await browser.close();
  process.exit(0);
}
if (process.argv.includes("--verify-restart")) {
  assert.equal(snapshot.accounts.length, 1);
  assert.equal(
    await command("copy_accounts", {
      ids: [snapshot.accounts[0].id],
      format: "cookie",
    }),
    "synthetic-cookie:tail",
  );
  assert.equal(snapshot.config.checkerConcurrency, 8);
  assert.equal(snapshot.config.checkerDelaySecs, 3);
  if (packaged) {
    assert(
      !page.url().includes("5173"),
      "Packaged frontend must not use the Vite server",
    );
    assert(
      snapshot.runtime.browserReady,
      "Managed runtime must be found beside the application",
    );
    await page.screenshot({
      path: "test-results/screenshots/native-windows-packaged.png",
    });
  }
  console.log(
    JSON.stringify({
      nativeRestart: "passed",
      windowsCredentialManager: "passed",
      encryptedSecretsReadable: "passed",
    }),
  );
  await page.getByRole("button", { name: "Close Nocta", exact: true }).click();
  await browser.close();
  process.exit(0);
}
assert.equal(
  snapshot.accounts.length,
  0,
  "Smoke test requires a fresh database",
);
await command("save_folder", {
  id: null,
  input: { name: "Smoke folder", color: "#a3a3a3", group: "Synthetic" },
});
snapshot = await command("get_snapshot");
const folderId = snapshot.folders[0].id;
const report = await command("import_accounts", {
  text: "SmokeAccount:synthetic-password:synthetic-cookie:tail",
  folderId,
  updateExisting: false,
});
assert.equal(report.added, 1);
snapshot = await command("get_snapshot");
const id = snapshot.accounts[0].id;
assert(!JSON.stringify(snapshot).includes("synthetic-cookie"));
assert(!JSON.stringify(snapshot).includes("synthetic-password"));
assert.equal(
  await command("copy_accounts", { ids: [id], format: "cookie" }),
  "synthetic-cookie:tail",
);
await Promise.all([
  command("patch_config", { patch: { checkerConcurrency: 8 } }),
  command("patch_config", { patch: { checkerDelaySecs: 3 } }),
]);
snapshot = await command("get_snapshot");
assert.equal(snapshot.config.checkerConcurrency, 8);
assert.equal(snapshot.config.checkerDelaySecs, 3);
await command("account_action", { ids: [id], action: "trash", folderId: null });
assert((await command("get_snapshot")).accounts[0].deletedAt);
await command("account_action", {
  ids: [id],
  action: "restore",
  folderId: null,
});
assert.equal((await command("get_snapshot")).accounts[0].deletedAt, null);
await command("delete_folder", { id: folderId });
assert.equal((await command("get_snapshot")).accounts[0].folderId, null);
await page.reload();
await page.waitForFunction(() => !!window.__TAURI_INTERNALS__);
assert.equal((await command("get_snapshot")).accounts.length, 1);
await page.screenshot({ path: "test-results/screenshots/native-windows.png" });
console.log(
  JSON.stringify({
    nativeIPC: "passed",
    keyringEncryption: "passed",
    concurrentPatches: "passed",
    importTrashRestoreFolders: "passed",
    snapshotSecrets: "excluded",
  }),
);
// Exit through the UI to verify the normal shutdown path.
await page.getByRole("button", { name: "Close Nocta", exact: true }).click();
await browser.close();
