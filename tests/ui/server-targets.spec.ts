import { expect, test } from "@playwright/test";

test("launch panel finds a game by name, picks a server and launches directly", async ({
  page,
}) => {
  await page.goto("/?benchmark=1");
  const snapshot = await page.evaluate(async () => {
    const module = await import("/src/lib/preview.ts");
    return module.previewSnapshot();
  });
  snapshot.runtime.platform = "windows";
  snapshot.runtime.capabilities = ["launch", "sessions"];
  snapshot.launcher.handlerRegistered = true;
  snapshot.launcher.multiInstanceSupported = true;
  snapshot.launcher.multiInstance = "acquired";
  snapshot.config.multiInstance = true;
  snapshot.config.placeId = "123";
  await page.addInitScript((initial) => {
    const state = structuredClone(initial);
    const callbacks = new Map();
    const listeners = new Set();
    let next = 0;
    const icon = (color) =>
      `data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32'%3E%3Crect width='32' height='32' fill='%23${color}'/%3E%3C/svg%3E`;
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
        if (command === "patch_config") {
          Object.assign(state.config, args.patch);
          state.revision++;
          for (const id of listeners)
            callbacks.get(id)?.({
              event: "nocta:snapshot",
              id,
              payload: structuredClone(state),
            });
          return;
        }
        if (command === "get_place_info")
          return {
            placeId: args.placeId,
            name: "Fixture game",
            universeId: "456",
            iconUrl: icon("66a58b"),
          };
        if (command === "search_games")
          return [
            {
              placeId: "91",
              universeId: "90",
              name: "Doors",
              creator: "LSPLASH",
              playing: 56575,
              iconUrl: icon("b05a3c"),
            },
            {
              placeId: "92",
              universeId: "93",
              name: "Doors But Bad",
              creator: null,
              playing: 43,
              iconUrl: null,
            },
          ];
        if (command === "list_servers")
          return {
            placeId: args.placeId,
            warnings: [],
            cancelled: false,
            nextCursor: null,
            items: [
              ...(args.includeVip
                ? [
                    {
                      id: "vip-0",
                      kind: "vip",
                      playing: 0,
                      maxPlayers: 12,
                      playerTokens: [],
                      name: "Grind spot",
                      targetId: "target-vip",
                      joinable: true,
                    },
                  ]
                : []),
              {
                id: "job-fixture",
                kind: "public",
                playing: 4,
                maxPlayers: 30,
                playerTokens: [],
                name: null,
                targetId: "target-fixture",
                joinable: true,
                ping: 41,
                fps: 59,
              },
              {
                id: "job-full",
                kind: "public",
                playing: 30,
                maxPlayers: 30,
                playerTokens: [],
                name: null,
                targetId: null,
                joinable: false,
              },
            ],
          };
        if (command === "cancel_server_list") return;
        if (command === "expand_invite_link")
          return args.url.includes("/share?")
            ? "https://www.roblox.com/games/777/?privateServerLinkCode=code-1"
            : args.url;
        if (command === "launch_game") return [1];
        throw new Error(`Unexpected fixture command: ${command}`);
      },
    };
  }, snapshot);
  await page.reload();
  const calls = (command: string) =>
    page.evaluate(
      (name) => window.fixtureCalls.filter((call) => call.command === name),
      command,
    );
  const lastPatch = () =>
    page.evaluate(
      () =>
        window.fixtureCalls
          .filter((call) => call.command === "patch_config")
          .at(-1)?.args.patch,
    );
  const targetButton = page.getByRole("button", {
    name: /^Change launch target/,
  });

  // The toolbar names the game and how accounts join it.
  await expect(targetButton).toContainText("Fixture game");
  await expect(targetButton).toContainText("Any server");
  await page
    .getByRole("checkbox", { name: "Select PreviewAccount_00001" })
    .check();
  await targetButton.click();
  const panel = page.getByRole("complementary", { name: "Launch" });
  const game = panel.getByRole("region", { name: "Game" });
  await expect(game.getByText("Fixture game", { exact: true })).toBeVisible();
  await game.getByRole("button", { name: "Save game", exact: true }).click();
  await expect(game.getByRole("list", { name: "Saved games" })).toContainText(
    "Fixture game",
  );

  // Find a game by name; the result already carries its icon, so no extra lookup.
  await game.getByLabel("Find a game").fill("doors");
  const results = game.getByRole("list", { name: "Search results" });
  await expect(results.getByRole("listitem")).toHaveCount(2);
  await expect(results.getByRole("listitem").first()).toContainText(
    "by LSPLASH · 56.6K playing",
  );
  await results.getByRole("listitem").first().click();
  await expect.poll(lastPatch).toEqual({ placeId: "91", inviteUrl: "" });
  await expect(game.locator(".launch-game-card strong")).toHaveText("Doors");
  expect(
    (await calls("get_place_info")).filter(
      (call) => call.args.placeId === "91",
    ),
  ).toHaveLength(0);
  await expect(game.getByRole("list", { name: "Recent games" })).toContainText(
    "Doors",
  );

  // Choosing a server loads the list right away; a click picks a row.
  const server = panel.getByRole("region", { name: "Server" });
  await expect(
    server.getByRole("radio", { name: /Any server/ }),
  ).toHaveAttribute("aria-checked", "true");
  await server.getByRole("radio", { name: /Choose a server/ }).click();
  const list = server.getByRole("radiogroup", { name: "Servers" });
  await expect(list.getByRole("radio")).toHaveCount(2);
  await expect(
    list.getByRole("radio", { name: "Server job-full" }),
  ).toBeDisabled();
  await list.getByRole("radio", { name: "Server job-fixture" }).click();
  await expect
    .poll(async () => (await lastPatch())?.inviteUrl)
    .toContain("/games/91/?gameInstanceId=job-fixture");
  await expect(
    list.getByRole("radio", { name: "Server job-fixture" }),
  ).toHaveAttribute("aria-checked", "true");
  await expect(panel.locator(".launch-footer-summary")).toHaveText(
    "Doors · Server job-fixt",
  );
  await page.screenshot({
    path: "test-results/screenshots/server-targets-1280.png",
  });

  // Launching from the toolbar goes straight to the chosen server: no extra dialog.
  await page.keyboard.press("Escape");
  await expect(panel).toBeHidden();
  await expect(targetButton).toContainText("Server job-fixt");
  await targetButton.screenshot({
    path: "test-results/screenshots/launch-target-button.png",
  });
  await page.getByRole("button", { name: /^Launch selected/ }).click();
  await expect.poll(async () => (await calls("launch_game")).length).toBe(1);
  const launched = (await calls("launch_game"))[0].args;
  expect(launched.ids).toHaveLength(1);
  expect(launched.target).toEqual({ kind: "configured" });
  await expect(page.getByRole("dialog")).toHaveCount(0);

  // Private servers of the selected account are listed on top and launched by their ID.
  await targetButton.click();
  await server
    .getByRole("checkbox", { name: "Private servers of PreviewAccount_00001" })
    .check();
  await list.getByRole("radio", { name: "Private server Grind spot" }).click();
  await expect(panel.locator(".launch-footer-summary")).toHaveText(
    "Doors · Private server · Grind spot",
  );
  await expect.poll(async () => (await lastPatch())?.inviteUrl).toBe("");
  await panel.getByRole("button", { name: "Launch 1 account" }).click();
  await expect(panel).toBeHidden();
  await expect.poll(async () => (await calls("launch_game")).length).toBe(2);
  expect((await calls("launch_game"))[1].args.target).toEqual({
    kind: "server",
    targetId: "target-vip",
  });

  // Back to any server, then a pasted game link switches the game without a search.
  await targetButton.click();
  await server.getByRole("radio", { name: /Any server/ }).click();
  await expect(targetButton).toContainText("Any server");
  await game
    .getByLabel("Find a game")
    .fill("https://www.roblox.com/games/555/Some-Game");
  await expect(game.getByRole("button", { name: /Place 555/ })).toBeVisible();
  await game.getByLabel("Find a game").press("Enter");
  await expect.poll(lastPatch).toEqual({ placeId: "555", inviteUrl: "" });
  expect(await calls("search_games")).toHaveLength(1);

  // A share link needs no account: Nocta expands it once and the game shows right away.
  await game
    .getByLabel("Find a game")
    .fill("https://www.roblox.com/share?code=2755&type=Server");
  await game.getByRole("button", { name: /Use this link/ }).click();
  await expect.poll(lastPatch).toEqual({
    placeId: "777",
    inviteUrl: "https://www.roblox.com/games/777/?privateServerLinkCode=code-1",
  });
  expect(await calls("resolve_launch_target")).toHaveLength(0);
  await expect(game.getByRole("alert")).toHaveCount(0);
  await expect(
    server.getByRole("radio", { name: /Invite link/ }),
  ).toHaveAttribute("aria-checked", "true");
  await page.keyboard.press("Escape");
  await expect(targetButton).toContainText("Private server link");
});

test("launching without a game opens the panel to pick one", async ({
  page,
}) => {
  await page.goto("/?benchmark=1");
  const snapshot = await page.evaluate(async () => {
    const module = await import("/src/lib/preview.ts");
    return module.previewSnapshot();
  });
  snapshot.runtime.capabilities = ["launch", "sessions"];
  snapshot.config.placeId = "";
  snapshot.config.inviteUrl = "";
  await page.addInitScript((initial) => {
    window.isTauri = true;
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    window.__TAURI_INTERNALS__ = {
      metadata: {
        currentWindow: { label: "main" },
        currentWebview: { label: "main" },
      },
      transformCallback: () => 1,
      unregisterCallback() {},
      async invoke(command) {
        if (command === "get_snapshot") return structuredClone(initial);
        if (command.startsWith("plugin:event|")) return;
        throw new Error(`Unexpected fixture command: ${command}`);
      },
    };
  }, snapshot);
  await page.reload();
  await expect(
    page.getByRole("button", { name: /^Change launch target/ }),
  ).toContainText("Choose a game");
  await page
    .getByRole("checkbox", { name: "Select PreviewAccount_00001" })
    .check();
  await page.getByRole("button", { name: /^Launch selected/ }).click();
  const panel = page.getByRole("complementary", { name: "Launch" });
  await expect(panel.getByRole("status")).toHaveText(
    "Choose a game first, then launch.",
  );
  await expect(panel.getByLabel("Find a game")).toBeFocused();
  await expect(
    panel.getByRole("button", { name: "Select accounts to launch" }),
  ).toHaveCount(0);
  await expect(
    panel.getByRole("button", { name: "Launch 1 account" }),
  ).toBeDisabled();
});
