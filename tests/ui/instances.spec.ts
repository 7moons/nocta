import { expect, test } from "@playwright/test";

test("lists outside Roblox clients and offers to close them when multi-instance is busy", async ({
  page,
}) => {
  await page.goto("/");
  const snapshot = await page.evaluate(async () => {
    const module = await import("/src/lib/preview.ts");
    return module.previewSnapshot();
  });
  snapshot.runtime.platform = "windows";
  snapshot.runtime.capabilities = ["launch", "sessions", "instances"];
  snapshot.launcher.handlerRegistered = true;
  snapshot.launcher.multiInstanceSupported = true;
  snapshot.launcher.multiInstance = "busy";
  snapshot.config.multiInstance = false;
  snapshot.externalInstances = [
    {
      id: "4100:1700000000000",
      pid: 4100,
      processName: "RobloxPlayerBeta.exe",
      startedAt: "2026-10-05T10:00:00Z",
    },
    {
      id: "4200:1700000001000",
      pid: 4200,
      processName: "RobloxPlayerBeta.exe",
      startedAt: "2026-10-05T10:01:00Z",
    },
  ];
  await page.addInitScript((initial) => {
    const state = structuredClone(initial);
    const callbacks = new Map();
    const listeners = new Set();
    let next = 0;
    const publish = () => {
      state.revision++;
      for (const id of listeners)
        callbacks.get(id)?.({
          event: "nocta:snapshot",
          id,
          payload: structuredClone(state),
        });
    };
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
        if (command === "plugin:event|listen") {
          listeners.add(args.handler);
          return args.handler;
        }
        if (command === "plugin:event|unlisten") return;
        if (command === "get_snapshot") return structuredClone(state);
        if (command === "close_external_instance") {
          state.externalInstances = state.externalInstances.filter(
            (item) => item.id !== args.id,
          );
          publish();
          return;
        }
        if (command === "close_all_roblox") {
          const closed = state.externalInstances.length;
          state.externalInstances = [];
          publish();
          return closed;
        }
        if (command === "patch_config") {
          Object.assign(state.config, args.patch);
          if (args.patch.multiInstance)
            state.launcher.multiInstance = "acquired";
          publish();
          return;
        }
        throw new Error(`Unexpected fixture command: ${command}`);
      },
    };
  }, snapshot);
  await page.reload();

  const dialog = page.getByRole("dialog", {
    name: "Multi-instance could not start",
  });
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText("2 Roblox clients are running");
  await page.screenshot({
    path: "test-results/screenshots/multi-instance-busy-1280.png",
  });
  await dialog.getByRole("button", { name: "Not now" }).click();
  await expect(dialog).toBeHidden();

  await page.getByRole("button", { name: /^Instances/ }).click();
  const outside = page.getByRole("list", {
    name: "Roblox clients started outside Nocta",
  });
  await expect(outside.getByRole("listitem")).toHaveCount(2);
  await page.screenshot({
    path: "test-results/screenshots/instances-external-1280.png",
  });
  page.once("dialog", (confirm) => void confirm.accept());
  await outside
    .getByRole("button", { name: "Close Roblox client PID 4100" })
    .click();
  await expect(outside.getByRole("listitem")).toHaveCount(1);
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.fixtureCalls.find(
            (call) => call.command === "close_external_instance",
          )?.args.id,
      ),
    )
    .toBe("4100:1700000000000");

  // Reopen the busy prompt (a later busy state) and resolve it in one step.
  await page.reload();
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Close all Roblox" }).click();
  await expect(dialog).toBeHidden();
  await expect(page.getByText("Multi-instance is on.")).toBeVisible();
  await expect(page.getByText("No Roblox clients running")).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.fixtureCalls
          .filter((call) =>
            ["close_all_roblox", "patch_config"].includes(call.command),
          )
          .map((call) => call.command),
      ),
    )
    .toEqual(["close_all_roblox", "patch_config"]);
});
