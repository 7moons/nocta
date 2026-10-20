import { expect, test } from "@playwright/test";

test("Roblox client settings install, pin, update mode and update notice", async ({
  page,
}) => {
  await page.goto("/");
  const snapshot = await page.evaluate(async () => {
    const module = await import("/src/lib/preview.ts");
    return module.previewSnapshot();
  });
  snapshot.runtime.platform = "windows";
  snapshot.config.managedClient = true;
  snapshot.config.clientUpdateMode = "ask";
  snapshot.client = {
    installed: [
      {
        guid: "version-1111111111111111",
        version: "0.740.0.1",
        installedAt: "2026-10-01T10:00:00Z",
        sizeMb: 330,
      },
      {
        guid: "version-0000000000000000",
        version: "0.739.0.1",
        installedAt: "2026-09-20T10:00:00Z",
        sizeMb: 329,
      },
    ],
    active: "version-1111111111111111",
    latest: {
      guid: "version-2222222222222222",
      version: "0.741.0.1",
      checkedAt: "2026-10-05T10:00:00Z",
    },
    progress: null,
    lastError: null,
    directory: "C:/Nocta/roblox",
  };
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
        if (command === "client_check_latest") return state.client.latest;
        if (command === "client_install") {
          // The backend reports progress through the snapshot while installing.
          state.client.progress = {
            guid: state.client.latest.guid,
            phase: "downloading",
            doneMb: 120,
            totalMb: 230,
            packagesDone: 7,
            packagesTotal: 21,
          };
          publish();
          return;
        }
        if (command === "client_remove") {
          state.client.installed = state.client.installed.filter(
            (item) => item.guid !== args.guid,
          );
          publish();
          return;
        }
        if (command === "patch_config") {
          Object.assign(state.config, args.patch);
          if ("clientPinnedVersion" in args.patch)
            state.client.active =
              args.patch.clientPinnedVersion ?? state.client.installed[0].guid;
          publish();
          return;
        }
        throw new Error(`Unexpected fixture command: ${command}`);
      },
    };
  }, snapshot);
  await page.reload();

  // "Ask first": an update notice appears over every view.
  const notice = page.getByRole("status").filter({ hasText: "is available" });
  await expect(notice).toContainText("Roblox 0.741.0.1 is available");

  await page.keyboard.press("Control+4");
  await page
    .getByRole("button", { name: "Roblox client", exact: true })
    .click();
  await expect(
    page.getByText("Roblox 0.740.0.1", { exact: true }),
  ).toBeVisible();
  await expect(page.getByText("Update available · 0.741.0.1")).toBeVisible();
  await page.screenshot({
    path: "test-results/screenshots/roblox-client-1280.png",
  });

  // Pin the older version, then follow the newest again.
  const versions = page.getByRole("list", { name: "Installed versions" });
  await versions.getByRole("button", { name: "Use this version" }).click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.fixtureCalls
            .filter((call) => call.command === "patch_config")
            .at(-1)?.args.patch.clientPinnedVersion,
      ),
    )
    .toBe("version-0000000000000000");
  await expect(versions.getByText("pinned")).toBeVisible();
  await page.getByRole("button", { name: "Follow the newest version" }).click();
  await expect(versions.getByText("pinned")).toHaveCount(0);

  // Removing a non-active version asks first.
  page.once("dialog", (confirm) => void confirm.accept());
  await versions
    .getByRole("button", { name: "Remove version-0000000000000000" })
    .click();
  await expect(versions.getByRole("listitem")).toHaveCount(1);

  await page.getByRole("switch", { name: "Low graphics" }).click();
  await page.getByLabel("Graphics API").selectOption("vulkan");
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.fixtureCalls
          .filter((call) => call.command === "patch_config")
          .map((call) => call.args.patch),
      ),
    )
    .toEqual(
      expect.arrayContaining([
        { clientLowGraphics: true },
        { clientGraphicsApi: "vulkan" },
      ]),
    );

  await page.getByLabel("Update mode").selectOption("manual");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.fixtureCalls
            .filter((call) => call.command === "patch_config")
            .at(-1)?.args.patch.clientUpdateMode,
      ),
    )
    .toBe("manual");

  await page.getByRole("button", { name: "Update to 0.741.0.1" }).click();
  await expect(
    page.getByRole("status").filter({ hasText: "Downloading" }),
  ).toContainText("120 / 230 MB");
  await expect(
    page.getByRole("status").filter({ hasText: "Installing Roblox" }),
  ).toContainText("52%");
  await expect(
    page.getByRole("button", { name: "Cancel install" }),
  ).toBeVisible();
  await page.screenshot({
    path: "test-results/screenshots/roblox-client-installing-1280.png",
  });
});
